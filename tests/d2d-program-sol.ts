import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { D2dProgramSol } from "../target/types/d2d_program_sol";
import { expect } from "chai";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import * as crypto from "crypto";

describe("D2D Program Tests", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.D2dProgramSol as Program<D2dProgramSol>;

  // Test accounts
  let admin: Keypair;
  let treasuryWallet: Keypair;
  let lender1: Keypair;
  let lender2: Keypair;
  let developer1: Keypair;
  let developer2: Keypair;
  let ephemeralKey: Keypair;

  // PDAs
  let treasuryPoolPDA: PublicKey;
  let treasuryPoolBump: number;

  // Constants
  const INITIAL_APY = 500; // 5% APY (in basis points)
  const SERVICE_FEE = 5 * LAMPORTS_PER_SOL; // 5 SOL
  const MONTHLY_FEE = 1 * LAMPORTS_PER_SOL; // 1 SOL
  const DEPLOYMENT_COST = 10 * LAMPORTS_PER_SOL; // 10 SOL

  before(async () => {
    // Generate test keypairs
    admin = Keypair.generate();
    treasuryWallet = Keypair.generate();
    lender1 = Keypair.generate();
    lender2 = Keypair.generate();
    developer1 = Keypair.generate();
    developer2 = Keypair.generate();
    ephemeralKey = Keypair.generate();

    // Airdrop SOL to test accounts
    await airdrop(admin.publicKey, 10 * LAMPORTS_PER_SOL);
    await airdrop(treasuryWallet.publicKey, 100 * LAMPORTS_PER_SOL);
    await airdrop(lender1.publicKey, 50 * LAMPORTS_PER_SOL);
    await airdrop(lender2.publicKey, 50 * LAMPORTS_PER_SOL);
    await airdrop(developer1.publicKey, 30 * LAMPORTS_PER_SOL);
    await airdrop(developer2.publicKey, 30 * LAMPORTS_PER_SOL);

    // Derive PDAs
    [treasuryPoolPDA, treasuryPoolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury_pool")],
      program.programId
    );
  });

  async function airdrop(publicKey: PublicKey, amount: number) {
    const signature = await provider.connection.requestAirdrop(publicKey, amount);
    await provider.connection.confirmTransaction(signature);
  }

  describe("1. Initialization", () => {
    it("Should initialize the treasury pool successfully", async () => {
      const tx = await program.methods
        .initialize(new anchor.BN(INITIAL_APY), treasuryWallet.publicKey)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      console.log("Initialize transaction signature:", tx);

      // Verify treasury pool state
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.admin.toString()).to.equal(admin.publicKey.toString());
      expect(treasuryPool.treasuryWallet.toString()).to.equal(treasuryWallet.publicKey.toString());
      expect(treasuryPool.currentApy.toNumber()).to.equal(INITIAL_APY);
      expect(treasuryPool.totalStaked.toNumber()).to.equal(0);
      expect(treasuryPool.emergencyPause).to.be.false;
    });

    it("Should fail to initialize twice", async () => {
      try {
        await program.methods
          .initialize(new anchor.BN(INITIAL_APY), treasuryWallet.publicKey)
          .accounts({
            treasuryPool: treasuryPoolPDA,
            admin: admin.publicKey,
            treasuryWallet: treasuryWallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error).to.exist;
      }
    });
  });

  describe("2. Lender Staking", () => {
    let lender1StakePDA: PublicKey;
    let lender2StakePDA: PublicKey;

    const STAKE_AMOUNT_1 = 20 * LAMPORTS_PER_SOL;
    const STAKE_AMOUNT_2 = 30 * LAMPORTS_PER_SOL;
    const LOCK_PERIOD = 30 * 24 * 60 * 60; // 30 days

    it("Lender 1 should stake SOL successfully", async () => {
      [lender1StakePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("lender_stake"), lender1.publicKey.toBuffer()],
        program.programId
      );

      const tx = await program.methods
        .stakeSol(new anchor.BN(STAKE_AMOUNT_1), new anchor.BN(LOCK_PERIOD))
        .accounts({
          treasuryPool: treasuryPoolPDA,
          lenderStake: lender1StakePDA,
          lender: lender1.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([lender1])
        .rpc();

      console.log("Lender 1 stake transaction:", tx);

      // Verify stake
      const stake = await program.account.lenderStake.fetch(lender1StakePDA);
      expect(stake.lender.toString()).to.equal(lender1.publicKey.toString());
      expect(stake.stakedAmount.toNumber()).to.equal(STAKE_AMOUNT_1);
      expect(stake.lockPeriod.toNumber()).to.equal(LOCK_PERIOD);
      expect(stake.isActive).to.be.true;

      // Verify treasury pool
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.totalStaked.toNumber()).to.equal(STAKE_AMOUNT_1);
    });

    it("Lender 2 should stake SOL successfully", async () => {
      [lender2StakePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("lender_stake"), lender2.publicKey.toBuffer()],
        program.programId
      );

      const tx = await program.methods
        .stakeSol(new anchor.BN(STAKE_AMOUNT_2), new anchor.BN(0)) // Flexible staking
        .accounts({
          treasuryPool: treasuryPoolPDA,
          lenderStake: lender2StakePDA,
          lender: lender2.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([lender2])
        .rpc();

      console.log("Lender 2 stake transaction:", tx);

      // Verify treasury pool
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.totalStaked.toNumber()).to.equal(STAKE_AMOUNT_1 + STAKE_AMOUNT_2);
    });

    it("Should fail to stake with insufficient amount", async () => {
      try {
        const lowAmount = 0.001 * LAMPORTS_PER_SOL;
        await program.methods
          .stakeSol(new anchor.BN(lowAmount), new anchor.BN(0))
          .accounts({
            treasuryPool: treasuryPoolPDA,
            lenderStake: lender1StakePDA,
            lender: lender1.publicKey,
            treasuryWallet: treasuryWallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([lender1])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error).to.exist;
      }
    });
  });

  describe("3. Program Deployment", () => {
    let deployRequestPDA: PublicKey;
    let userStatsPDA: PublicKey;
    let programHash: Buffer;

    before(() => {
      // Generate a unique program hash for testing
      programHash = crypto.randomBytes(32);
    });

    it("Developer should deploy program successfully", async () => {
      [deployRequestPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("deploy_request"), programHash],
        program.programId
      );

      [userStatsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_stats"), developer1.publicKey.toBuffer()],
        program.programId
      );

      const initialMonths = 3;
      const totalPayment = SERVICE_FEE + (MONTHLY_FEE * initialMonths);

      const tx = await program.methods
        .deployProgram(
          Array.from(programHash),
          new anchor.BN(SERVICE_FEE),
          new anchor.BN(MONTHLY_FEE),
          initialMonths,
          new anchor.BN(DEPLOYMENT_COST)
        )
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          developer: developer1.publicKey,
          admin: admin.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          ephemeralKey: ephemeralKey.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([developer1, admin])
        .rpc();

      console.log("Deploy program transaction:", tx);

      // Verify deployment request
      const deployRequest = await program.account.deployRequest.fetch(deployRequestPDA);
      expect(deployRequest.developer.toString()).to.equal(developer1.publicKey.toString());
      expect(deployRequest.serviceFee.toNumber()).to.equal(SERVICE_FEE);
      expect(deployRequest.monthlyFee.toNumber()).to.equal(MONTHLY_FEE);
      expect(deployRequest.deploymentCost.toNumber()).to.equal(DEPLOYMENT_COST);
      expect(deployRequest.status).to.deep.equal({ pendingDeployment: {} });

      // Verify user stats
      const userStats = await program.account.userDeployStats.fetch(userStatsPDA);
      expect(userStats.totalDeploys.toNumber()).to.equal(1);
      expect(userStats.activeSessions).to.equal(1);

      // Verify ephemeral key received deployment cost
      const ephemeralBalance = await provider.connection.getBalance(ephemeralKey.publicKey);
      expect(ephemeralBalance).to.be.gte(DEPLOYMENT_COST);
    });

    it("Should fail deployment when program is paused", async () => {
      // First pause the program
      await program.methods
        .emergencyPause(true)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      try {
        const newProgramHash = crypto.randomBytes(32);
        const [newDeployRequestPDA] = PublicKey.findProgramAddressSync(
          [Buffer.from("deploy_request"), newProgramHash],
          program.programId
        );

        await program.methods
          .deployProgram(
            Array.from(newProgramHash),
            new anchor.BN(SERVICE_FEE),
            new anchor.BN(MONTHLY_FEE),
            3,
            new anchor.BN(DEPLOYMENT_COST)
          )
          .accounts({
            treasuryPool: treasuryPoolPDA,
            deployRequest: newDeployRequestPDA,
            userStats: userStatsPDA,
            developer: developer1.publicKey,
            admin: admin.publicKey,
            treasuryWallet: treasuryWallet.publicKey,
            ephemeralKey: ephemeralKey.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([developer1, admin])
          .rpc();

        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("ProgramPaused");
      }

      // Unpause for next tests
      await program.methods
        .emergencyPause(false)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();
    });

    it("Admin should confirm deployment success", async () => {
      const deployedProgramId = Keypair.generate().publicKey;

      const tx = await program.methods
        .confirmDeploymentSuccess(Array.from(programHash), deployedProgramId)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      console.log("Confirm deployment success transaction:", tx);

      // Verify deployment request status
      const deployRequest = await program.account.deployRequest.fetch(deployRequestPDA);
      expect(deployRequest.status).to.deep.equal({ active: {} });
      expect(deployRequest.deployedProgramId.toString()).to.equal(deployedProgramId.toString());

      // Verify user stats
      const userStats = await program.account.userDeployStats.fetch(userStatsPDA);
      expect(userStats.activeSessions).to.equal(0); // Decremented after confirmation
    });
  });

  describe("4. Subscription Payment", () => {
    let programHash: Buffer;
    let deployRequestPDA: PublicKey;
    let userStatsPDA: PublicKey;

    before(async () => {
      // Create a new deployment for subscription testing
      programHash = crypto.randomBytes(32);

      [deployRequestPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("deploy_request"), programHash],
        program.programId
      );

      [userStatsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_stats"), developer2.publicKey.toBuffer()],
        program.programId
      );

      // Deploy a program first
      await program.methods
        .deployProgram(
          Array.from(programHash),
          new anchor.BN(SERVICE_FEE),
          new anchor.BN(MONTHLY_FEE),
          1,
          new anchor.BN(DEPLOYMENT_COST)
        )
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          developer: developer2.publicKey,
          admin: admin.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          ephemeralKey: Keypair.generate().publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([developer2, admin])
        .rpc();

      // Confirm deployment
      await program.methods
        .confirmDeploymentSuccess(Array.from(programHash), Keypair.generate().publicKey)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();
    });

    it("Developer should pay subscription successfully", async () => {
      const monthsToPay = 6;

      // Get subscription before payment
      const deployRequestBefore = await program.account.deployRequest.fetch(deployRequestPDA);
      const subscriptionBefore = deployRequestBefore.subscriptionPaidUntil.toNumber();

      const tx = await program.methods
        .paySubscription(Array.from(programHash), monthsToPay)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          developer: developer2.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([developer2])
        .rpc();

      console.log("Pay subscription transaction:", tx);

      // Verify subscription extension
      const deployRequestAfter = await program.account.deployRequest.fetch(deployRequestPDA);
      const subscriptionAfter = deployRequestAfter.subscriptionPaidUntil.toNumber();
      
      const expectedExtension = monthsToPay * 30 * 24 * 60 * 60;
      expect(subscriptionAfter).to.be.gte(subscriptionBefore + expectedExtension - 5); // Allow 5 second tolerance
    });
  });

  describe("5. Lender Rewards & Unstaking", () => {
    let lender2StakePDA: PublicKey;

    before(async () => {
      [lender2StakePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("lender_stake"), lender2.publicKey.toBuffer()],
        program.programId
      );

      // Wait a bit for rewards to accumulate (in real scenarios, this would be longer)
      await new Promise(resolve => setTimeout(resolve, 2000));
    });

    it("Lender should claim rewards", async () => {
      const stakeBefore = await program.account.lenderStake.fetch(lender2StakePDA);
      const balanceBefore = await provider.connection.getBalance(lender2.publicKey);

      try {
        const tx = await program.methods
          .claimRewards()
          .accounts({
            treasuryPool: treasuryPoolPDA,
            lenderStake: lender2StakePDA,
            lender: lender2.publicKey,
            treasuryWallet: treasuryWallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([lender2])
          .rpc();

        console.log("Claim rewards transaction:", tx);

        const stakeAfter = await program.account.lenderStake.fetch(lender2StakePDA);
        expect(stakeAfter.totalClaimed.toNumber()).to.be.gte(stakeBefore.totalClaimed.toNumber());
      } catch (error) {
        // It's OK if there are no rewards yet (NoRewardsToClaim error)
        if (!error.toString().includes("NoRewardsToClaim")) {
          throw error;
        }
        console.log("No rewards to claim yet (expected for quick test)");
      }
    });

    it("Lender should unstake SOL successfully", async () => {
      const unstakeAmount = 10 * LAMPORTS_PER_SOL;

      const tx = await program.methods
        .unstakeSol(new anchor.BN(unstakeAmount))
        .accounts({
          treasuryPool: treasuryPoolPDA,
          lenderStake: lender2StakePDA,
          lender: lender2.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([lender2])
        .rpc();

      console.log("Unstake transaction:", tx);

      // Verify stake amount reduced
      const stake = await program.account.lenderStake.fetch(lender2StakePDA);
      expect(stake.stakedAmount.toNumber()).to.equal(20 * LAMPORTS_PER_SOL);
    });
  });

  describe("6. Admin Functions", () => {
    it("Admin should update APY", async () => {
      const newAPY = 750; // 7.5%

      const tx = await program.methods
        .updateApy(new anchor.BN(newAPY))
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      console.log("Update APY transaction:", tx);

      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.currentApy.toNumber()).to.equal(newAPY);
    });

    it("Should fail when non-admin tries to update APY", async () => {
      try {
        await program.methods
          .updateApy(new anchor.BN(1000))
          .accounts({
            treasuryPool: treasuryPoolPDA,
            admin: developer1.publicKey,
          })
          .signers([developer1])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error).to.exist;
      }
    });

    it("Admin should toggle emergency pause", async () => {
      // Pause
      await program.methods
        .emergencyPause(true)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      let treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.emergencyPause).to.be.true;

      // Unpause
      await program.methods
        .emergencyPause(false)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      expect(treasuryPool.emergencyPause).to.be.false;
    });
  });

  describe("7. Deployment Failure Handling", () => {
    let programHash: Buffer;
    let deployRequestPDA: PublicKey;
    let userStatsPDA: PublicKey;

    before(async () => {
      programHash = crypto.randomBytes(32);

      [deployRequestPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("deploy_request"), programHash],
        program.programId
      );

      [userStatsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_stats"), developer1.publicKey.toBuffer()],
        program.programId
      );

      // Deploy a program
      await program.methods
        .deployProgram(
          Array.from(programHash),
          new anchor.BN(SERVICE_FEE),
          new anchor.BN(MONTHLY_FEE),
          2,
          new anchor.BN(DEPLOYMENT_COST)
        )
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          developer: developer1.publicKey,
          admin: admin.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          ephemeralKey: Keypair.generate().publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([developer1, admin])
        .rpc();
    });

    it("Admin should confirm deployment failure with refund", async () => {
      const failureReason = "Program verification failed";

      const developerBalanceBefore = await provider.connection.getBalance(developer1.publicKey);

      const tx = await program.methods
        .confirmDeploymentFailure(Array.from(programHash), failureReason)
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: deployRequestPDA,
          userStats: userStatsPDA,
          admin: admin.publicKey,
          developer: developer1.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      console.log("Confirm deployment failure transaction:", tx);

      // Verify deployment request status
      const deployRequest = await program.account.deployRequest.fetch(deployRequestPDA);
      expect(deployRequest.status).to.deep.equal({ failed: {} });

      // Verify developer received refund
      const developerBalanceAfter = await provider.connection.getBalance(developer1.publicKey);
      expect(developerBalanceAfter).to.be.gt(developerBalanceBefore);
    });
  });

  describe("8. Edge Cases & Security", () => {
    it("Should handle multiple deployments from same developer", async () => {
      const [userStatsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_stats"), developer1.publicKey.toBuffer()],
        program.programId
      );

      const statsBefore = await program.account.userDeployStats.fetch(userStatsPDA);
      const totalDeploysBefore = statsBefore.totalDeploys.toNumber();

      // Create another deployment
      const newProgramHash = crypto.randomBytes(32);
      const [newDeployRequestPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("deploy_request"), newProgramHash],
        program.programId
      );

      await program.methods
        .deployProgram(
          Array.from(newProgramHash),
          new anchor.BN(SERVICE_FEE),
          new anchor.BN(MONTHLY_FEE),
          1,
          new anchor.BN(DEPLOYMENT_COST)
        )
        .accounts({
          treasuryPool: treasuryPoolPDA,
          deployRequest: newDeployRequestPDA,
          userStats: userStatsPDA,
          developer: developer1.publicKey,
          admin: admin.publicKey,
          treasuryWallet: treasuryWallet.publicKey,
          ephemeralKey: Keypair.generate().publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([developer1, admin])
        .rpc();

      const statsAfter = await program.account.userDeployStats.fetch(userStatsPDA);
      expect(statsAfter.totalDeploys.toNumber()).to.equal(totalDeploysBefore + 1);
    });

    it("Should fail with insufficient treasury funds", async () => {
      try {
        const newProgramHash = crypto.randomBytes(32);
        const [newDeployRequestPDA] = PublicKey.findProgramAddressSync(
          [Buffer.from("deploy_request"), newProgramHash],
          program.programId
        );

        const [userStatsPDA] = PublicKey.findProgramAddressSync(
          [Buffer.from("user_stats"), developer2.publicKey.toBuffer()],
          program.programId
        );

        // Try to deploy with extremely high deployment cost
        const hugeDeploymentCost = 1000000 * LAMPORTS_PER_SOL;

        await program.methods
          .deployProgram(
            Array.from(newProgramHash),
            new anchor.BN(SERVICE_FEE),
            new anchor.BN(MONTHLY_FEE),
            1,
            new anchor.BN(hugeDeploymentCost)
          )
          .accounts({
            treasuryPool: treasuryPoolPDA,
            deployRequest: newDeployRequestPDA,
            userStats: userStatsPDA,
            developer: developer2.publicKey,
            admin: admin.publicKey,
            treasuryWallet: treasuryWallet.publicKey,
            ephemeralKey: Keypair.generate().publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([developer2, admin])
          .rpc();

        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("InsufficientTreasuryFunds");
      }
    });
  });

  describe("9. Final State Verification", () => {
    it("Should have correct final treasury state", async () => {
      const treasuryPool = await program.account.treasuryPool.fetch(treasuryPoolPDA);
      
      console.log("\n=== Final Treasury State ===");
      console.log("Total Staked:", treasuryPool.totalStaked.toNumber() / LAMPORTS_PER_SOL, "SOL");
      console.log("Total Fees Collected:", treasuryPool.totalFeesCollected.toNumber() / LAMPORTS_PER_SOL, "SOL");
      console.log("Total Rewards Distributed:", treasuryPool.totalRewardsDistributed.toNumber() / LAMPORTS_PER_SOL, "SOL");
      console.log("Current APY:", treasuryPool.currentApy.toNumber() / 100, "%");
      console.log("Emergency Pause:", treasuryPool.emergencyPause);

      expect(treasuryPool.totalFeesCollected.toNumber()).to.be.gt(0);
    });
  });
});
