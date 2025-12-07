import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Multidistribute } from "../target/types/multidistribute";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
  getAssociatedTokenAddress,
  getMint,
} from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { assert } from "chai";

describe("multidistribute", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Multidistribute as Program<Multidistribute>;
  
  // We'll use these accounts throughout the tests
  let mint1: PublicKey;
  let mint2: PublicKey;
  let authorityTokenAccount1: PublicKey;
  let authorityTokenAccount2: PublicKey;
  let userTokenAccount1: PublicKey;
  let userTokenAccount2: PublicKey;
  let collection: PublicKey;
  let collectionVault: PublicKey;
  let replacementMint: PublicKey;
  let userState: PublicKey;
  let userState1: PublicKey;
  let userReplacementTokenAccount: PublicKey;
  let distribution1: PublicKey;
  let distribution1Vault: PublicKey;
  let distribution2: PublicKey;
  let distribution2Vault: PublicKey;
  let distribution1UserState: PublicKey;
  let distribution2UserState: PublicKey;
  let distribution2UserState1: PublicKey;

  const user = anchor.web3.Keypair.generate();
  const authority = provider.wallet;
  const MAX_TOKENS = new anchor.BN(1000);
  const COUNTER = new anchor.BN(1);

  before(async () => {
    // Airdrop SOL to user
    const signature = await provider.connection.requestAirdrop(
      user.publicKey,
      1000000000
    );
    await provider.connection.confirmTransaction(signature);

    // Create mint and token accounts
    mint1 = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey,
      null,
      6
    );

    authorityTokenAccount1 = await createAccount(
      provider.connection,
      authority.payer,
      mint1,
      authority.publicKey
    );

    userTokenAccount1 = await createAccount(
      provider.connection,
      authority.payer,
      mint1,
      user.publicKey
    );

    // Mint some tokens to authority and user
    // Create second mint and accounts
    mint2 = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey,
      null,
      6
    );

    authorityTokenAccount2 = await createAccount(
      provider.connection,
      authority.payer,
      mint2,
      authority.publicKey
    );

    userTokenAccount2 = await createAccount(
      provider.connection,
      authority.payer,
      mint2,
      user.publicKey
    );

    // Mint tokens for both mints
    await mintTo(
      provider.connection,
      authority.payer,
      mint1,
      authorityTokenAccount1,
      authority.publicKey,
      10000
    );

    await mintTo(
      provider.connection,
      authority.payer,
      mint1,
      userTokenAccount1,
      authority.publicKey,
      10000
    );

    await mintTo(
      provider.connection,
      authority.payer,
      mint2,
      authorityTokenAccount2,
      authority.publicKey,
      10000
    );

    await mintTo(
      provider.connection,
      authority.payer,
      mint2,
      userTokenAccount2,
      authority.publicKey,
      10000
    );

    // Derive PDAs
    [collection] = await PublicKey.findProgramAddress(
      [
        Buffer.from("collection"),
        authority.publicKey.toBuffer(),
        mint1.toBuffer(),
        COUNTER.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    collectionVault = await getAssociatedTokenAddress(
      mint1,
      collection,
      true // allowOwnerOffCurve: true since collection is a PDA
    );

    [replacementMint] = await PublicKey.findProgramAddress(
      [
        Buffer.from("replacement_mint"),
        collection.toBuffer(),
      ],
      program.programId
    );

    userReplacementTokenAccount = await getAssociatedTokenAddress(
      replacementMint,
      user.publicKey
    );

    [userState] = await PublicKey.findProgramAddress(
      [
        Buffer.from("user_state"),
        collection.toBuffer(),
        user.publicKey.toBuffer(),
        (new anchor.BN(0)).toArrayLike(Buffer, 'le', 8),
      ],
      program.programId
    );

    [userState1] = await PublicKey.findProgramAddress(
      [
        Buffer.from("user_state"),
        collection.toBuffer(),
        user.publicKey.toBuffer(),
        (new anchor.BN(1)).toArrayLike(Buffer, 'le', 8),
      ],
      program.programId
    );

    [distribution1] = await PublicKey.findProgramAddress(
      [
        Buffer.from("distribution"),
        collection.toBuffer(),
        mint1.toBuffer(),
      ],
      program.programId
    );

    distribution1Vault = await getAssociatedTokenAddress(
      mint1,
      distribution1,
      true // allowOwnerOffCurve: true since distribution1 is a PDA
    );

    [distribution2] = await PublicKey.findProgramAddress(
      [
        Buffer.from("distribution"),
        collection.toBuffer(),
        mint2.toBuffer(),
      ],
      program.programId
    );

    distribution2Vault = await getAssociatedTokenAddress(
      mint2,
      distribution2,
      true // allowOwnerOffCurve: true since distribution2 is a PDA
    );

    [distribution1UserState] = await PublicKey.findProgramAddress(
      [
        Buffer.from("distribution_user_state"),
        distribution1.toBuffer(),
        userState.toBuffer(),
      ],
      program.programId
    );

    [distribution2UserState] = await PublicKey.findProgramAddress(
      [
        Buffer.from("distribution_user_state"),
        distribution2.toBuffer(),
        userState.toBuffer(),
      ],
      program.programId
    );

    [distribution2UserState1] = await PublicKey.findProgramAddress(
      [
        Buffer.from("distribution_user_state"),
        distribution2.toBuffer(),
        userState1.toBuffer(),
      ],
      program.programId
    );
  });

  it("Creates a collection", async () => {
    await program.methods
      .initCollection(COUNTER, MAX_TOKENS)
      .accounts({
        collection,
        mint: mint1,
        vault: collectionVault,
        replacementMint,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const collectionAccount = await program.account.collection.fetch(collection);
    assert.equal(collectionAccount.authority.toString(), authority.publicKey.toString());
    assert.equal(collectionAccount.lifetimeTokensCollected.toString(), "0");
    assert.equal(collectionAccount.maxCollectableTokens.toString(), MAX_TOKENS.toString());
  });

  it("Creates distributions", async () => {
    // Create first distribution
    await program.methods
      .initDistribution()
      .accounts({
        distribution: distribution1,
        collection,
        mint: mint1,
        vault: distribution1Vault,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Add tokens to first distribution
    await program.methods
      .addDistributionTokens(new anchor.BN(100))
      .accounts({
        distribution: distribution1,
        collection,
        vault: distribution1Vault,
        authorityTokenAccount: authorityTokenAccount1,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Create second distribution
    await program.methods
      .initDistribution()
      .accounts({
        distribution: distribution2,
        collection,
        mint: mint2,
        vault: distribution2Vault,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Add tokens to second distribution
    await program.methods
      .addDistributionTokens(new anchor.BN(200))
      .accounts({
        distribution: distribution2,
        collection,
        vault: distribution2Vault,
        authorityTokenAccount: authorityTokenAccount2,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const distribution1Account = await program.account.distribution.fetch(distribution1);
    const distribution2Account = await program.account.distribution.fetch(distribution2);

    assert.equal(distribution1Account.lifetimeDepositedTokens.toString(), "100");
    assert.equal(distribution2Account.lifetimeDepositedTokens.toString(), "200");

    // Finish setup
    await program.methods
      .finalizeCollectionSetup()
      .accounts({
        collection,
        authority: authority.publicKey,
      })
      .rpc();
  });

  it("Deposits tokens and receives distributions", async () => {
    // Check initial balances
    const userAccount1BeforeCommit = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const vaultBeforeCommit = await getAccount(
      provider.connection,
      collectionVault
    );

    // First commit to collection
    await program.methods
      .userCommitToCollection(new anchor.BN(0), new anchor.BN(500))
      .accounts({
        collection,
        userState,
        mint: mint1,
        userTokenAccount: userTokenAccount1,
        vault: collectionVault,
        replacementMint,
        userReplacementTokenAccount,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([user])
      .rpc();

    // Verify balances after commit
    const userAccount1AfterCommit = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const vaultAfterCommit = await getAccount(
      provider.connection,
      collectionVault
    );
    
    assert.equal(
      userAccount1BeforeCommit.amount - BigInt(500),
      userAccount1AfterCommit.amount
    );
    assert.equal(
      vaultBeforeCommit.amount + BigInt(500),
      vaultAfterCommit.amount
    );

    // Check balances before claim
    const userAccount1BeforeClaim = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const distribution1VaultBeforeClaim = await getAccount(
      provider.connection,
      distribution1Vault
    );

    // Claim from first distribution
    await program.methods
      .userClaimFromDistribution()
      .accounts({
        collection,
        distribution: distribution1,
        collectionUserState: userState,
        distributionUserState: distribution1UserState,
        distributionVault: distribution1Vault,
        userTokenAccount: userTokenAccount1,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // Verify balances after claim
    const userAccount1AfterClaim = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const distribution1VaultAfterClaim = await getAccount(
      provider.connection,
      distribution1Vault
    );

    assert.equal(
      userAccount1BeforeClaim.amount + BigInt(50),
      userAccount1AfterClaim.amount
    );
    assert.equal(
      distribution1VaultBeforeClaim.amount - BigInt(50),
      distribution1VaultAfterClaim.amount
    );

    // Verify received amount (should be 50 tokens - half of distribution1)
    const distribution1UserStateAccount = await program.account.distributionUserState.fetch(
      distribution1UserState
    );
    assert.equal(distribution1UserStateAccount.receivedAmount.toString(), "50");

    // Check balances before second commit
    const userAccount1BeforeCommit2 = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const vaultBeforeCommit2 = await getAccount(
      provider.connection,
      collectionVault
    );

    // Second commit to collection
    await program.methods
      .userCommitToCollection(new anchor.BN(1), new anchor.BN(300))
      .accounts({
        collection,
        userState: userState1,
        mint: mint1,
        userTokenAccount: userTokenAccount1,
        vault: collectionVault,
        replacementMint,
        userReplacementTokenAccount,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([user])
      .rpc();

    // Verify balances after second commit
    const userAccount1AfterCommit2 = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const vaultAfterCommit2 = await getAccount(
      provider.connection,
      collectionVault
    );

    assert.equal(
      userAccount1BeforeCommit2.amount - BigInt(300),
      userAccount1AfterCommit2.amount
    );
    assert.equal(
      vaultBeforeCommit2.amount + BigInt(300),
      vaultAfterCommit2.amount
    );

    // Check balances before second claim
    const userAccount2BeforeClaim = await getAccount(
      provider.connection,
      userTokenAccount2
    );
    const distribution2VaultBeforeClaim = await getAccount(
      provider.connection,
      distribution2Vault
    );

    // Claim from second distribution
    await program.methods
      .userClaimFromDistribution()
      .accounts({
        collection,
        distribution: distribution2,
        collectionUserState: userState,
        distributionUserState: distribution2UserState,
        distributionVault: distribution2Vault,
        userTokenAccount: userTokenAccount2,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();
    // Claim from second distribution
    await program.methods
      .userClaimFromDistribution()
      .accounts({
        collection,
        distribution: distribution2,
        collectionUserState: userState1,
        distributionUserState: distribution2UserState1,
        distributionVault: distribution2Vault,
        userTokenAccount: userTokenAccount2,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // Verify balances after second claim
    const userAccount2AfterClaim = await getAccount(
      provider.connection,
      userTokenAccount2
    );
    const distribution2VaultAfterClaim = await getAccount(
      provider.connection,
      distribution2Vault
    );

    assert.equal(
      userAccount2BeforeClaim.amount + BigInt(160),
      userAccount2AfterClaim.amount
    );
    assert.equal(
      distribution2VaultBeforeClaim.amount - BigInt(160),
      distribution2VaultAfterClaim.amount
    );

    // Verify received amount (should be 100 + 60 tokens - 80% of distribution2)
    const distribution2UserStateAccount = await program.account.distributionUserState.fetch(
      distribution2UserState
    );
    assert.equal(distribution2UserStateAccount.receivedAmount.toString(), "100");
    const distribution2UserStateAccount1 = await program.account.distributionUserState.fetch(
      distribution2UserState1
    );
    assert.equal(distribution2UserStateAccount1.receivedAmount.toString(), "60");
  });

  it("Withdraws tokens from collection", async () => {
    // Check balances before withdrawal
    const vaultBeforeWithdraw = await getAccount(
      provider.connection,
      collectionVault
    );
    const authorityAccount1BeforeWithdraw = await getAccount(
      provider.connection,
      authorityTokenAccount1
    );

    await program.methods
      .withdrawFromCollection()
      .accounts({
        collection,
        vault: collectionVault,
        authorityTokenAccount: authorityTokenAccount1,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Verify balances after withdrawal
    const vaultAfterWithdraw = await getAccount(
      provider.connection,
      collectionVault
    );
    const authorityAccount1AfterWithdraw = await getAccount(
      provider.connection,
      authorityTokenAccount1
    );

    // All tokens (800) should be withdrawn
    assert.equal(vaultAfterWithdraw.amount, BigInt(0));
    assert.equal(
      authorityAccount1AfterWithdraw.amount,
      authorityAccount1BeforeWithdraw.amount + BigInt(800)
    );
  });
});
