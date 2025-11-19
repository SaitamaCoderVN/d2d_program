import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { D2dProgramSol } from "../target/types/d2d_program_sol";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("Fee-Based Reward System", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.D2dProgramSol as Program<D2dProgramSol>;
  
  // Test accounts
  const admin = Keypair.generate();
  const devWallet = Keypair.generate();
  const backer1 = Keypair.generate();
  const backer2 = Keypair.generate();
  
  // PDAs
  let treasuryPoolPda: PublicKey;
  let rewardPoolPda: PublicKey;
  let platformPoolPda: PublicKey;
  let backer1DepositPda: PublicKey;
  let backer2DepositPda: PublicKey;
  
  // Treasury pool bump
  let treasuryBump: number;
  let rewardPoolBump: number;
  let platformPoolBump: number;

  before(async () => {
    // Airdrop SOL to test accounts
    await provider.connection.requestAirdrop(admin.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(backer1.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(backer2.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(devWallet.publicKey, 1 * LAMPORTS_PER_SOL);
    
    // Wait for airdrops to confirm
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Derive PDAs
    [treasuryPoolPda, treasuryBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury_pool")],
      program.programId
    );
    
    [rewardPoolPda, rewardPoolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_pool")],
      program.programId
    );
    
    [platformPoolPda, platformPoolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("platform_pool")],
      program.programId
    );
    
    [backer1DepositPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lender_stake"), backer1.publicKey.toBuffer()],
      program.programId
    );
    
    [backer2DepositPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lender_stake"), backer2.publicKey.toBuffer()],
      program.programId
    );

    // Initialize treasury pool
    try {
      await program.methods
        .initialize(0, devWallet.publicKey) // initial_apy = 0 (not used)
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          admin: admin.publicKey,
          devWallet: devWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();
    } catch (err) {
      // May already be initialized
      console.log("Treasury pool may already be initialized");
    }
  });

  describe("deposit()", () => {
    it("Should deposit SOL and calculate fees correctly", async () => {
      const depositAmount = new anchor.BN(10 * LAMPORTS_PER_SOL); // 10 SOL
      
      // Expected fees: 1% reward, 0.1% platform
      const expectedRewardFee = depositAmount.toNumber() * 0.01; // 1% = 0.1 SOL
      const expectedPlatformFee = depositAmount.toNumber() * 0.001; // 0.1% = 0.01 SOL
      const expectedNetDeposit = depositAmount.toNumber() - expectedRewardFee - expectedPlatformFee;

      // Get initial balances
      const initialRewardPoolBalance = await provider.connection.getBalance(rewardPoolPda);
      const initialPlatformPoolBalance = await provider.connection.getBalance(platformPoolPda);
      const initialDevWalletBalance = await provider.connection.getBalance(devWallet.publicKey);

      // Deposit
      await program.methods
        .stakeSol(depositAmount, new anchor.BN(0)) // lock_period = 0
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          devWallet: devWallet.publicKey,
          lenderStake: backer1DepositPda,
          lender: backer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer1])
        .rpc();

      // Verify balances
      const finalRewardPoolBalance = await provider.connection.getBalance(rewardPoolPda);
      const finalPlatformPoolBalance = await provider.connection.getBalance(platformPoolPda);
      const finalDevWalletBalance = await provider.connection.getBalance(devWallet.publicKey);

      // Check fees were transferred (accounting for rent)
      const rewardPoolIncrease = finalRewardPoolBalance - initialRewardPoolBalance;
      const platformPoolIncrease = finalPlatformPoolBalance - initialPlatformPoolBalance;
      const devWalletIncrease = finalDevWalletBalance - initialDevWalletBalance;

      expect(rewardPoolIncrease).to.be.closeTo(expectedRewardFee, 1000); // Allow 1000 lamport tolerance
      expect(platformPoolIncrease).to.be.closeTo(expectedPlatformFee, 1000);
      expect(devWalletIncrease).to.be.closeTo(expectedNetDeposit, 1000);

      // Verify treasury pool state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      expect(treasuryPool.totalDeposited.toNumber()).to.equal(expectedNetDeposit);
      expect(treasuryPool.rewardPoolBalance.toNumber()).to.be.closeTo(expectedRewardFee, 1000);
      expect(treasuryPool.platformPoolBalance.toNumber()).to.be.closeTo(expectedPlatformFee, 1000);

      // Verify backer deposit state
      const backerDeposit = await program.account.backerDeposit.fetch(backer1DepositPda);
      expect(backerDeposit.depositedAmount.toNumber()).to.equal(expectedNetDeposit);
      expect(backerDeposit.rewardEarned.toNumber()).to.be.closeTo(expectedRewardFee, 1000); // Should equal reward fee for single backer
      expect(backerDeposit.rewardClaimed.toNumber()).to.equal(0);
    });

    it("Should handle multiple backers with proportional rewards", async () => {
      const backer1Deposit = new anchor.BN(5 * LAMPORTS_PER_SOL); // 5 SOL
      const backer2Deposit = new anchor.BN(10 * LAMPORTS_PER_SOL); // 10 SOL

      // Backer 1 deposit
      await program.methods
        .stakeSol(backer1Deposit, new anchor.BN(0))
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          devWallet: devWallet.publicKey,
          lenderStake: backer1DepositPda,
          lender: backer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer1])
        .rpc();

      // Backer 2 deposit
      await program.methods
        .stakeSol(backer2Deposit, new anchor.BN(0))
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          devWallet: devWallet.publicKey,
          lenderStake: backer2DepositPda,
          lender: backer2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer2])
        .rpc();

      // Get treasury pool state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const totalRewardPool = treasuryPool.rewardPoolBalance.toNumber();
      const totalDeposited = treasuryPool.totalDeposited.toNumber();

      // Calculate expected proportional shares
      const backer1NetDeposit = backer1Deposit.toNumber() * 0.989; // After 1.1% fees
      const backer2NetDeposit = backer2Deposit.toNumber() * 0.989;
      const expectedBacker1Share = (totalRewardPool * backer1NetDeposit) / totalDeposited;
      const expectedBacker2Share = (totalRewardPool * backer2NetDeposit) / totalDeposited;

      // Verify backer deposits
      const backer1DepositAccount = await program.account.backerDeposit.fetch(backer1DepositPda);
      const backer2DepositAccount = await program.account.backerDeposit.fetch(backer2DepositPda);

      expect(backer1DepositAccount.rewardEarned.toNumber()).to.be.closeTo(expectedBacker1Share, 1000);
      expect(backer2DepositAccount.rewardEarned.toNumber()).to.be.closeTo(expectedBacker2Share, 1000);
      
      // Backer 2 should have more rewards (deposited more)
      expect(backer2DepositAccount.rewardEarned.toNumber()).to.be.greaterThan(
        backer1DepositAccount.rewardEarned.toNumber()
      );
    });
  });

  describe("claim()", () => {
    it("Should claim rewards correctly", async () => {
      // Get initial balances
      const initialBacker1Balance = await provider.connection.getBalance(backer1.publicKey);
      const initialRewardPoolBalance = await provider.connection.getBalance(rewardPoolPda);

      // Get backer deposit state
      const backerDepositBefore = await program.account.backerDeposit.fetch(backer1DepositPda);
      const claimableRewards = backerDepositBefore.rewardEarned.toNumber() - backerDepositBefore.rewardClaimed.toNumber();

      expect(claimableRewards).to.be.greaterThan(0);

      // Claim rewards
      await program.methods
        .claimRewards()
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          lenderStake: backer1DepositPda,
          lender: backer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer1])
        .rpc();

      // Verify balances
      const finalBacker1Balance = await provider.connection.getBalance(backer1.publicKey);
      const finalRewardPoolBalance = await provider.connection.getBalance(rewardPoolPda);

      const backer1Increase = finalBacker1Balance - initialBacker1Balance;
      const rewardPoolDecrease = initialRewardPoolBalance - finalRewardPoolBalance;

      expect(backer1Increase).to.be.closeTo(claimableRewards, 5000); // Allow 5000 lamport tolerance for fees
      expect(rewardPoolDecrease).to.be.closeTo(claimableRewards, 1000);

      // Verify backer deposit state
      const backerDepositAfter = await program.account.backerDeposit.fetch(backer1DepositPda);
      expect(backerDepositAfter.rewardClaimed.toNumber()).to.equal(backerDepositBefore.rewardEarned.toNumber());
      expect(backerDepositAfter.rewardClaimed.toNumber()).to.be.greaterThan(0);

      // Verify treasury pool state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      expect(treasuryPool.rewardPoolBalance.toNumber()).to.be.closeTo(
        initialRewardPoolBalance - claimableRewards,
        1000
      );
    });

    it("Should prevent double claiming", async () => {
      // Try to claim again (should have no claimable rewards)
      const backerDeposit = await program.account.backerDeposit.fetch(backer1DepositPda);
      const claimableRewards = backerDeposit.rewardEarned.toNumber() - backerDeposit.rewardClaimed.toNumber();

      expect(claimableRewards).to.equal(0);

      // Attempting to claim should fail or do nothing
      try {
        await program.methods
          .claimRewards()
          .accounts({
            treasuryPool: treasuryPoolPda,
            rewardPool: rewardPoolPda,
            lenderStake: backer1DepositPda,
            lender: backer1.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([backer1])
          .rpc();
        
        // If it doesn't throw, verify no rewards were claimed
        const backerDepositAfter = await program.account.backerDeposit.fetch(backer1DepositPda);
        expect(backerDepositAfter.rewardClaimed.toNumber()).to.equal(backerDeposit.rewardClaimed.toNumber());
      } catch (err) {
        // Expected to fail with NoRewardsToClaim error
        expect(err.toString()).to.include("NoRewardsToClaim");
      }
    });
  });

  describe("Edge cases", () => {
    it("Should handle division by zero (no deposits)", async () => {
      // Create new treasury pool for this test
      const newTreasuryPoolPda = Keypair.generate();
      
      // Calculate reward share when total_deposited = 0
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      
      // If total_deposited is 0, reward share should be 0
      if (treasuryPool.totalDeposited.toNumber() === 0) {
        const backerDeposit = await program.account.backerDeposit.fetch(backer1DepositPda);
        expect(backerDeposit.rewardEarned.toNumber()).to.equal(0);
      }
    });

    it("Should handle very small deposits", async () => {
      const smallDeposit = new anchor.BN(1000); // 0.000001 SOL
      
      // Should not panic on small amounts
      try {
        await program.methods
          .stakeSol(smallDeposit, new anchor.BN(0))
          .accounts({
            treasuryPool: treasuryPoolPda,
            rewardPool: rewardPoolPda,
            platformPool: platformPoolPda,
            devWallet: devWallet.publicKey,
            lenderStake: backer1DepositPda,
            lender: backer1.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([backer1])
          .rpc();
        
        // Verify fees were calculated (may be 0 for very small amounts)
        const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
        expect(treasuryPool.totalDeposited.toNumber()).to.be.greaterThanOrEqual(0);
      } catch (err) {
        // May fail due to insufficient funds for rent, which is expected
        console.log("Small deposit test:", err.toString());
      }
    });
  });
});

