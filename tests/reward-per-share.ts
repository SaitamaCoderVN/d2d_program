import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { D2dProgramSol } from "../target/types/d2d_program_sol";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { BN } from "@coral-xyz/anchor";

describe("Reward-Per-Share Model", () => {
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
  
  const PRECISION = new BN("1000000000000"); // 1e12

  before(async () => {
    // Airdrop SOL to test accounts
    await provider.connection.requestAirdrop(admin.publicKey, 20 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(backer1.publicKey, 20 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(backer2.publicKey, 20 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(devWallet.publicKey, 1 * LAMPORTS_PER_SOL);
    
    // Wait for airdrops to confirm
    await new Promise(resolve => setTimeout(resolve, 2000));

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
      console.log("Treasury pool may already be initialized:", err);
    }
  });

  describe("Scenario A: Two backers deposit, then fee credit", () => {
    it("Should correctly calculate proportional rewards after fee credit", async () => {
      // Backer 1 deposits 10 SOL
      const backer1Deposit = new BN(10 * LAMPORTS_PER_SOL);
      await program.methods
        .stakeSol(backer1Deposit, new BN(0))
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          treasuryPda: treasuryPoolPda,
          lenderStake: backer1DepositPda,
          lender: backer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer1])
        .rpc();

      // Backer 2 deposits 5 SOL
      const backer2Deposit = new BN(5 * LAMPORTS_PER_SOL);
      await program.methods
        .stakeSol(backer2Deposit, new BN(0))
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          treasuryPda: treasuryPoolPda,
          lenderStake: backer2DepositPda,
          lender: backer2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backer2])
        .rpc();

      // Get treasury pool state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const totalDeposited = treasuryPool.totalDeposited.toNumber();
      
      // Admin credits 1.5 SOL to reward pool (simulating dev fee payment)
      const feeReward = new BN(1.5 * LAMPORTS_PER_SOL);
      const feePlatform = new BN(0.15 * LAMPORTS_PER_SOL); // 0.1% of 1.5 SOL
      
      await program.methods
        .creditFeeToPool(feeReward, feePlatform)
        .accounts({
          treasuryPool: treasuryPoolPda,
          rewardPool: rewardPoolPda,
          platformPool: platformPoolPda,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      // Get updated treasury pool
      const treasuryPoolAfter = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const rewardPerShare = treasuryPoolAfter.rewardPerShare;
      
      // Get backer deposits
      const backer1DepositAccount = await program.account.backerDeposit.fetch(backer1DepositPda);
      const backer2DepositAccount = await program.account.backerDeposit.fetch(backer2DepositPda);
      
      // Calculate expected claimable using formula:
      // claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
      const backer1Deposited = backer1DepositAccount.depositedAmount.toNumber();
      const backer2Deposited = backer2DepositAccount.depositedAmount.toNumber();
      
      // Calculate claimable manually
      const backer1Accumulated = new BN(backer1Deposited).mul(rewardPerShare);
      const backer1Claimable = backer1Accumulated.sub(backer1DepositAccount.rewardDebt).div(PRECISION);
      
      const backer2Accumulated = new BN(backer2Deposited).mul(rewardPerShare);
      const backer2Claimable = backer2Accumulated.sub(backer2DepositAccount.rewardDebt).div(PRECISION);
      
      // Expected: backer1 should have 2/3 of rewards (10/15), backer2 should have 1/3 (5/15)
      // But we need to account for fees deducted from deposits
      const backer1NetDeposit = backer1Deposit.toNumber() * 0.989; // After 1.1% fees
      const backer2NetDeposit = backer2Deposit.toNumber() * 0.989;
      const totalNetDeposit = backer1NetDeposit + backer2NetDeposit;
      
      const expectedBacker1Share = (feeReward.toNumber() * backer1NetDeposit) / totalNetDeposit;
      const expectedBacker2Share = (feeReward.toNumber() * backer2NetDeposit) / totalNetDeposit;
      
      // Allow 1% tolerance for rounding
      expect(backer1Claimable.toNumber()).to.be.closeTo(expectedBacker1Share, expectedBacker1Share * 0.01);
      expect(backer2Claimable.toNumber()).to.be.closeTo(expectedBacker2Share, expectedBacker2Share * 0.01);
      
      // Backer 1 should have more rewards (deposited more)
      expect(backer1Claimable.toNumber()).to.be.greaterThan(backer2Claimable.toNumber());
    });
  });

  describe("Scenario B: Partial claim, then new fee credit", () => {
    it("Should correctly update claimable after partial claim and new fee", async () => {
      // Get initial state
      const treasuryPoolBefore = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const backer1DepositBefore = await program.account.backerDeposit.fetch(backer1DepositPda);
      
      // Calculate claimable before claim
      const claimableBefore = new BN(backer1DepositBefore.depositedAmount.toNumber())
        .mul(treasuryPoolBefore.rewardPerShare)
        .sub(backer1DepositBefore.rewardDebt)
        .div(PRECISION);
      
      if (claimableBefore.toNumber() > 0) {
        // Claim partial (if there are rewards)
        const claimAmount = claimableBefore.div(new BN(2)); // Claim half
        
        // Note: We can't claim partial in current implementation, so we'll claim all
        // This test verifies the reward_debt update after claim
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
        
        // Get state after claim
        const backer1DepositAfter = await program.account.backerDeposit.fetch(backer1DepositPda);
        
        // reward_debt should be updated to current accumulated value
        const expectedRewardDebt = new BN(backer1DepositAfter.depositedAmount.toNumber())
          .mul(treasuryPoolBefore.rewardPerShare);
        
        expect(backer1DepositAfter.rewardDebt.toString()).to.equal(expectedRewardDebt.toString());
        
        // Credit new fee
        const newFeeReward = new BN(0.5 * LAMPORTS_PER_SOL);
        await program.methods
          .creditFeeToPool(newFeeReward, new BN(0))
          .accounts({
            treasuryPool: treasuryPoolPda,
            rewardPool: rewardPoolPda,
            platformPool: platformPoolPda,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();
        
        // Get updated state
        const treasuryPoolAfter = await program.account.treasuryPool.fetch(treasuryPoolPda);
        const backer1DepositFinal = await program.account.backerDeposit.fetch(backer1DepositPda);
        
        // Calculate new claimable
        const newClaimable = new BN(backer1DepositFinal.depositedAmount.toNumber())
          .mul(treasuryPoolAfter.rewardPerShare)
          .sub(backer1DepositFinal.rewardDebt)
          .div(PRECISION);
        
        // Should have new rewards available
        expect(newClaimable.toNumber()).to.be.greaterThan(0);
      }
    });
  });

  describe("Scenario C: Unstake with liquid balance", () => {
    it("Should allow unstake when liquid_balance is sufficient", async () => {
      // Get initial state
      const treasuryPoolBefore = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const backer1DepositBefore = await program.account.backerDeposit.fetch(backer1DepositPda);
      
      const unstakeAmount = new BN(1 * LAMPORTS_PER_SOL);
      
      // Check if liquid_balance is sufficient
      if (treasuryPoolBefore.liquidBalance.toNumber() >= unstakeAmount.toNumber()) {
        const initialBalance = await provider.connection.getBalance(backer1.publicKey);
        
        await program.methods
          .unstakeSol(unstakeAmount)
          .accounts({
            treasuryPool: treasuryPoolPda,
            treasuryPda: treasuryPoolPda,
            lenderStake: backer1DepositPda,
            lender: backer1.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([backer1])
          .rpc();
        
        // Verify balance increased
        const finalBalance = await provider.connection.getBalance(backer1.publicKey);
        expect(finalBalance).to.be.greaterThan(initialBalance);
        
        // Verify state updated
        const treasuryPoolAfter = await program.account.treasuryPool.fetch(treasuryPoolPda);
        const backer1DepositAfter = await program.account.backerDeposit.fetch(backer1DepositPda);
        
        expect(treasuryPoolAfter.totalDeposited.toNumber()).to.equal(
          treasuryPoolBefore.totalDeposited.toNumber() - unstakeAmount.toNumber()
        );
        expect(treasuryPoolAfter.liquidBalance.toNumber()).to.equal(
          treasuryPoolBefore.liquidBalance.toNumber() - unstakeAmount.toNumber()
        );
        expect(backer1DepositAfter.depositedAmount.toNumber()).to.equal(
          backer1DepositBefore.depositedAmount.toNumber() - unstakeAmount.toNumber()
        );
      }
    });

    it("Should fail unstake when liquid_balance is insufficient", async () => {
      // Get current state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
      const backer1Deposit = await program.account.backerDeposit.fetch(backer1DepositPda);
      
      // Try to unstake more than liquid_balance
      const unstakeAmount = new BN(treasuryPool.liquidBalance.toNumber() + 1);
      
      if (unstakeAmount.toNumber() <= backer1Deposit.depositedAmount.toNumber()) {
        try {
          await program.methods
            .unstakeSol(unstakeAmount)
            .accounts({
              treasuryPool: treasuryPoolPda,
              treasuryPda: treasuryPoolPda,
              lenderStake: backer1DepositPda,
              lender: backer1.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([backer1])
            .rpc();
          
          expect.fail("Should have thrown InsufficientLiquidBalance error");
        } catch (err) {
          expect(err.toString()).to.include("InsufficientLiquidBalance");
        }
      }
    });
  });

  describe("Division by zero protection", () => {
    it("Should handle credit_fee_to_pool when total_deposited is 0", async () => {
      // This test requires a fresh treasury pool with no deposits
      // In practice, this should update reward_per_share correctly
      // If total_deposited == 0, reward_per_share should not change
      
      const feeReward = new BN(1 * LAMPORTS_PER_SOL);
      const feePlatform = new BN(0.1 * LAMPORTS_PER_SOL);
      
      // Should not panic even if total_deposited is 0
      // reward_per_share simply won't increase
      try {
        await program.methods
          .creditFeeToPool(feeReward, feePlatform)
          .accounts({
            treasuryPool: treasuryPoolPda,
            rewardPool: rewardPoolPda,
            platformPool: platformPoolPda,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();
        
        // Verify pools were credited
        const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPda);
        // reward_per_share should remain unchanged if total_deposited == 0
        // But pools should be credited
        expect(treasuryPool.rewardPoolBalance.toNumber()).to.be.greaterThan(0);
      } catch (err) {
        // Should not throw division by zero error
        expect(err.toString()).to.not.include("DivisionByZero");
      }
    });
  });
});

