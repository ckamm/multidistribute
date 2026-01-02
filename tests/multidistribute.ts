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
  let userReplacementTokenAccount: PublicKey;
  let distribution1: PublicKey;
  let distribution1Vault: PublicKey;
  let distribution2: PublicKey;
  let distribution2Vault: PublicKey;

  const user = anchor.web3.Keypair.generate();
  const authority = provider.wallet;
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

    // Create replacement mint (pre-created, not a PDA)
    // Must have: decimals matching mint1, authority as mint_authority, no freeze_authority
    replacementMint = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey, // mintAuthority - will be transferred to collection PDA
      null, // freezeAuthority - must be null
      6 // decimals - must match mint1
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

    userReplacementTokenAccount = await getAssociatedTokenAddress(
      replacementMint,
      user.publicKey
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
  });

  it("Creates a collection", async () => {
    await program.methods
      .initCollection(COUNTER, false, false)
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
    // maxCollectableTokens is now set from mint supply (10000 + 10000 = 20000)
    assert.equal(collectionAccount.maxCollectableTokens.toString(), "20000");
    assert.equal(collectionAccount.burnInsteadOfWithdraw, false);
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
        vault: distribution1Vault,
        depositorTokenAccount: authorityTokenAccount1,
        depositor: authority.publicKey,
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
        vault: distribution2Vault,
        depositorTokenAccount: authorityTokenAccount2,
        depositor: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const distribution1Account = await program.account.distribution.fetch(distribution1);
    const distribution2Account = await program.account.distribution.fetch(distribution2);
    
    assert.equal(distribution1Account.lifetimeDepositedTokens.toString(), "100");
    assert.equal(distribution2Account.lifetimeDepositedTokens.toString(), "200");
  });

  it("Commits and claims from multiple distributions statelessly", async () => {
    // Check balances before stateless commit and claim
    const userAccount1Before = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const userAccount2Before = await getAccount(
      provider.connection,
      userTokenAccount2
    );
    const vaultBefore = await getAccount(
      provider.connection,
      collectionVault
    );
    const dist1VaultBefore = await getAccount(
      provider.connection,
      distribution1Vault
    );
    const dist2VaultBefore = await getAccount(
      provider.connection,
      distribution2Vault
    );

    // User commits 200 tokens (1% of 20000 max) and claims from both distributions
    // Expected: receive 1 from dist1 (1% of 100) and 2 from dist2 (1% of 200)
    const COMMIT_AMOUNT = new anchor.BN(200);

    await program.methods
      .userCommitAndClaimStateless(COMMIT_AMOUNT)
      .accounts({
        collection,
        mint: mint1,
        userTokenAccount: userTokenAccount1,
        vault: collectionVault,
        replacementMint: replacementMint,
        userReplacementTokenAccount: userReplacementTokenAccount,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([
        // Distribution 1 accounts
        { pubkey: distribution1, isWritable: true, isSigner: false },
        { pubkey: distribution1Vault, isWritable: true, isSigner: false },
        { pubkey: userTokenAccount1, isWritable: true, isSigner: false },
        // Distribution 2 accounts
        { pubkey: distribution2, isWritable: true, isSigner: false },
        { pubkey: distribution2Vault, isWritable: true, isSigner: false },
        { pubkey: userTokenAccount2, isWritable: true, isSigner: false },
      ])
      .signers([user])
      .rpc();

    // Verify balances after stateless commit and claim
    const userAccount1After = await getAccount(
      provider.connection,
      userTokenAccount1
    );
    const userAccount2After = await getAccount(
      provider.connection,
      userTokenAccount2
    );
    const vaultAfter = await getAccount(
      provider.connection,
      collectionVault
    );
    const dist1VaultAfter = await getAccount(
      provider.connection,
      distribution1Vault
    );
    const dist2VaultAfter = await getAccount(
      provider.connection,
      distribution2Vault
    );

    // User committed 200 tokens to collection, but also received 1 back from dist1
    assert.equal(
      userAccount1Before.amount - BigInt(199),
      userAccount1After.amount
    );

    // User received tokens from dist2
    assert.equal(
      userAccount2Before.amount + BigInt(2),
      userAccount2After.amount
    );

    // Collection vault should have tokens
    assert.equal(
      vaultBefore.amount + BigInt(200),
      vaultAfter.amount
    );

    // Distribution 1 vault should have fewer tokens
    assert.equal(
      dist1VaultBefore.amount - BigInt(1),
      dist1VaultAfter.amount
    );

    // Distribution 2 vault should have fewer tokens
    assert.equal(
      dist2VaultBefore.amount - BigInt(2),
      dist2VaultAfter.amount
    );

    // Verify replacement tokens were minted
    const userReplacementBalance = (await getAccount(
      provider.connection,
      userReplacementTokenAccount
    )).amount;
    assert.equal(userReplacementBalance, BigInt(200));

    // Verify distribution states were updated
    const dist1Account = await program.account.distribution.fetch(distribution1);
    const dist2Account = await program.account.distribution.fetch(distribution2);
    assert.equal(dist1Account.distributedTokens.toString(), "1");
    assert.equal(dist2Account.distributedTokens.toString(), "2");
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
        mint: mint1,
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
      authorityAccount1BeforeWithdraw.amount + BigInt(200)
    );
  });
});

describe("multidistribute - burn_instead_of_withdraw", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Multidistribute as Program<Multidistribute>;

  let mint: PublicKey;
  let authorityTokenAccount: PublicKey;
  let userTokenAccount: PublicKey;
  let collection: PublicKey;
  let collectionVault: PublicKey;
  let replacementMint: PublicKey;
  let userReplacementTokenAccount: PublicKey;

  const user = anchor.web3.Keypair.generate();
  const authority = provider.wallet;
  const COUNTER = new anchor.BN(2); // Different counter to avoid collision

  before(async () => {
    // Airdrop SOL to user
    const signature = await provider.connection.requestAirdrop(
      user.publicKey,
      1000000000
    );
    await provider.connection.confirmTransaction(signature);

    // Create mint and token accounts
    mint = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey,
      null,
      6
    );

    authorityTokenAccount = await createAccount(
      provider.connection,
      authority.payer,
      mint,
      authority.publicKey
    );

    userTokenAccount = await createAccount(
      provider.connection,
      authority.payer,
      mint,
      user.publicKey
    );

    // Mint tokens
    await mintTo(
      provider.connection,
      authority.payer,
      mint,
      authorityTokenAccount,
      authority.publicKey,
      10000
    );

    await mintTo(
      provider.connection,
      authority.payer,
      mint,
      userTokenAccount,
      authority.publicKey,
      10000
    );

    // Create replacement mint
    replacementMint = await createMint(
      provider.connection,
      authority.payer,
      authority.publicKey,
      null,
      6
    );

    // Derive PDAs
    [collection] = await PublicKey.findProgramAddress(
      [
        Buffer.from("collection"),
        authority.publicKey.toBuffer(),
        mint.toBuffer(),
        COUNTER.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    collectionVault = await getAssociatedTokenAddress(
      mint,
      collection,
      true
    );

    userReplacementTokenAccount = await getAssociatedTokenAddress(
      replacementMint,
      user.publicKey
    );
  });

  it("Creates a collection with burn_instead_of_withdraw=true", async () => {
    await program.methods
      .initCollection(COUNTER, false, true) // burn_instead_of_withdraw = true
      .accounts({
        collection,
        mint: mint,
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
    assert.equal(collectionAccount.burnInsteadOfWithdraw, true);
  });

  it("User commits tokens to collection", async () => {
    const COMMIT_AMOUNT = new anchor.BN(500);

    await program.methods
      .userCommitAndClaimStateless(COMMIT_AMOUNT)
      .accounts({
        collection,
        mint: mint,
        userTokenAccount: userTokenAccount,
        vault: collectionVault,
        replacementMint: replacementMint,
        userReplacementTokenAccount: userReplacementTokenAccount,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .remainingAccounts([]) // No distributions
      .signers([user])
      .rpc();

    const vaultBalance = await getAccount(provider.connection, collectionVault);
    assert.equal(vaultBalance.amount, BigInt(500));
  });

  it("Burns tokens instead of withdrawing when flag is set", async () => {
    const mintInfoBefore = await getMint(provider.connection, mint);
    const vaultBefore = await getAccount(provider.connection, collectionVault);
    const authorityAccountBefore = await getAccount(
      provider.connection,
      authorityTokenAccount
    );

    await program.methods
      .withdrawFromCollection()
      .accounts({
        collection,
        mint: mint,
        vault: collectionVault,
        authorityTokenAccount: authorityTokenAccount,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const mintInfoAfter = await getMint(provider.connection, mint);
    const vaultAfter = await getAccount(provider.connection, collectionVault);
    const authorityAccountAfter = await getAccount(
      provider.connection,
      authorityTokenAccount
    );

    // Vault should be empty
    assert.equal(vaultAfter.amount, BigInt(0));

    // Authority account should NOT have received the tokens
    assert.equal(authorityAccountAfter.amount, authorityAccountBefore.amount);

    // Mint supply should have decreased (tokens were burned)
    assert.equal(
      mintInfoAfter.supply,
      mintInfoBefore.supply - vaultBefore.amount
    );
  });
});
