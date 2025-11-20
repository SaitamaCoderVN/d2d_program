# D2D Program - Sequence Diagrams

This document contains Mermaid sequence diagrams for all major flows in the D2D (Developer-to-Deploy) platform.

## Table of Contents

1. [Backer Staking Flow](#1-backer-staking-flow)
2. [Developer Deployment Flow](#2-developer-deployment-flow)
3. [Reward Claiming Flow](#3-reward-claiming-flow)
4. [Unstake Flow](#4-unstake-flow)
5. [Admin Operations](#5-admin-operations)

---

## 1. Backer Staking Flow

Flow when a backer stakes SOL into the treasury pool.

```mermaid
sequenceDiagram
    participant B as Backer (Frontend)
    participant F as Frontend (Next.js)
    participant W as Solana Wallet
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant S as BackerDeposit PDA

    B->>F: Enter stake amount & click "Stake"
    F->>F: Validate amount (min 0.1 SOL)
    F->>W: Check wallet balance
    W-->>F: Balance + rent exemption estimate
    
    alt Insufficient balance
        F-->>B: Show error: "Insufficient balance"
    else Sufficient balance
        F->>W: Request transaction signature
        W->>F: Sign transaction
        
        Note over F,P: Transaction includes:<br/>- stake_sol instruction<br/>- deposit_amount (100% to Treasury, NO fees from backer)
        
        F->>P: Send stake_sol transaction
        P->>P: Validate: !emergency_pause, amount > 0
        P->>P: Check lender balance (deposit + rent + fees)
        
        alt Insufficient balance
            P-->>F: Error: "InsufficientDeposit"
        else Sufficient balance
            P->>P: Initialize/update BackerDeposit account
            
            alt New backer
                P->>S: Initialize BackerDeposit PDA
                Note over S: deposited_amount = 0<br/>reward_debt = 0<br/>is_active = true
            else Existing backer
                P->>S: Update reward_debt (settle pending rewards)
                Note over S: reward_debt = deposited_amount * reward_per_share
            end
            
            P->>P: Update: deposited_amount += deposit_amount
            P->>T: Transfer deposit_amount (100% to Treasury, NO fees)
            P->>P: Update: total_deposited += deposit_amount
            P->>P: Update: liquid_balance += deposit_amount
            P->>P: Update: reward_debt = deposited_amount * reward_per_share
        
        P->>P: Emit SolStaked event
        P-->>F: Transaction confirmed
        F->>F: Refresh on-chain data
        F-->>B: Show success: "Successfully staked X SOL"
    end
```

---

## 2. Developer Deployment Flow

Complete flow from developer payment to program deployment.

```mermaid
sequenceDiagram
    participant D as Developer (Frontend)
    participant F as Frontend (Next.js)
    participant W as Solana Wallet
    participant BE as Backend (NestJS)
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant R as Reward Pool PDA
    participant PP as Platform Pool PDA
    participant DR as DeployRequest PDA
    participant EK as Ephemeral Wallet
    participant SC as Solana CLI

    D->>F: Fill deployment form & click "Pay & Deploy"
    F->>F: Calculate costs (service fee + monthly fee + platform fee)
    F->>F: Get Reward Pool & Platform Pool PDA addresses
    F->>W: Request payment transaction (2 transfers)
    W->>F: Sign payment transaction
    
    Note over F,R: Payment split:<br/>1. monthlyFee (1%) + serviceFee → RewardPool<br/>2. platformFee (0.1%) → PlatformPool
    
    F->>R: Transfer 1: monthlyFee + serviceFee (off-chain)
    F->>PP: Transfer 2: platformFee (off-chain)
    R-->>F: Payment 1 confirmed
    PP-->>F: Payment 2 confirmed
    F->>BE: POST /api/deployments/execute<br/>(paymentSignature, programHash, ...)
    
    BE->>BE: Verify payment transaction (2 transfers)
    BE->>BE: Check: Transfer 1: to == RewardPool, amount == monthlyFee + serviceFee
    BE->>BE: Check: Transfer 2: to == PlatformPool, amount == platformFee
    
    alt Payment verification failed
        BE-->>F: Error: "Payment verification failed"
    else Payment verified
        BE->>P: create_deploy_request instruction
        
        Note over BE,P: Admin-only instruction<br/>Payments already in pools
        
        P->>P: Validate: !emergency_pause, fees > 0
        P->>DR: Initialize/update DeployRequest PDA
        P->>P: Update: reward_pool_balance += monthlyFee + serviceFee
        P->>P: Update: platform_pool_balance += platformFee
        P->>P: Update: reward_per_share (if deposits exist)
        P->>P: Set status = PendingDeployment
        P-->>BE: DeployRequest created
        
        BE->>BE: Create deployment record in DB
        BE->>BE: Generate ephemeral keypair
        BE->>P: fund_temporary_wallet instruction
        
        Note over BE,P: Admin-only instruction<br/>Funds from TreasuryPool.liquid_balance<br/>(NOT RewardPool or PlatformPool)
        
        P->>P: Check: liquid_balance >= deployment_cost
        P->>T: Transfer deployment_cost to ephemeral wallet
        P->>P: Update: liquid_balance -= deployment_cost
        P->>P: Update: borrowed_amount = deployment_cost
        P->>DR: Update: ephemeral_key = ephemeral_wallet
        P-->>BE: Temporary wallet funded
        
        BE->>SC: Deploy program using Solana CLI
        SC->>SC: Build program
        SC->>SC: Deploy to Solana network
        SC-->>BE: Deployment result
        
        alt Deployment failed
            BE->>P: confirm_deployment_failure instruction
            P->>R: Refund payment to developer
            P->>P: Update: reward_pool_balance -= refund
            P->>T: Return deployment_cost to liquid_balance
            P->>DR: Set status = Failed
            P-->>BE: Failure confirmed
            BE-->>F: Error: "Deployment failed"
        else Deployment succeeded
            BE->>EK: Sweep remaining funds (if any)
            EK-->>BE: Recovered funds
            
            BE->>P: confirm_deployment_success instruction
            
            Note over BE,P: recovered_funds = min(requested, actual)
            
            P->>P: Validate: status == PendingDeployment
            P->>DR: Set status = Active
            P->>DR: Set deployed_program_id
            
            alt Recovered funds > 0
                P->>T: Transfer recovered_funds to Treasury Pool
                P->>P: Update: liquid_balance += recovered_funds
                Note over P: Recovered funds go to TreasuryPool<br/>(NOT PlatformPool)
            end
            
            P->>P: Emit DeploymentConfirmed event
            P-->>BE: Success confirmed
            
            BE->>BE: Update deployment record: status = Active
            BE-->>F: Success: "Deployment completed"
            F-->>D: Show success message
        end
    end
```

---

## 3. Reward Claiming Flow

Flow when a backer claims their accumulated rewards.

```mermaid
sequenceDiagram
    participant B as Backer (Frontend)
    participant F as Frontend (Next.js)
    participant W as Solana Wallet
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant R as Reward Pool PDA
    participant S as BackerDeposit PDA

    B->>F: Click "Claim Rewards"
    F->>F: Fetch on-chain data
    F->>F: Calculate claimable rewards
    
    Note over F: claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
    
    alt No claimable rewards
        F-->>B: Show: "No rewards to claim"
    else Has claimable rewards
        F->>W: Request transaction signature
        W->>F: Sign transaction
        
        Note over F,P: Transaction includes:<br/>- claim_rewards instruction
        
        F->>P: Send claim_rewards transaction
        P->>P: Validate: !emergency_pause
        P->>S: Fetch BackerDeposit account
        P->>T: Fetch TreasuryPool account
        
        P->>P: Calculate claimable_rewards
        Note over P: claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
        
        P->>P: Check: reward_pool_balance >= claimable_rewards
        
        alt Insufficient reward pool balance
            P-->>F: Error: "Insufficient reward pool balance"
        else Sufficient balance
            P->>R: Transfer claimable_rewards to backer
            P->>P: Update: reward_pool_balance -= claimable_rewards
            P->>P: Update: total_rewards_distributed += claimable_rewards
            P->>S: Update: reward_debt = deposited_amount * reward_per_share
            P->>S: Update: claimed_total += claimable_rewards
            
            P->>P: Emit Claimed event
            P-->>F: Transaction confirmed
            F->>F: Refresh on-chain data
            F-->>B: Show success: "Successfully claimed X SOL"
        end
    end
```

---

## 4. Unstake Flow

Flow when a backer unstakes (withdraws) their principal SOL.

```mermaid
sequenceDiagram
    participant B as Backer (Frontend)
    participant F as Frontend (Next.js)
    participant W as Solana Wallet
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant S as BackerDeposit PDA

    B->>F: Enter unstake amount & click "Unstake"
    F->>F: Validate amount (<= deposited_amount)
    
    alt Amount > deposited_amount
        F-->>B: Show error: "Amount exceeds stake"
    else Valid amount
        F->>W: Request transaction signature
        W->>F: Sign transaction
        
        Note over F,P: Transaction includes:<br/>- unstake_sol instruction
        
        F->>P: Send unstake_sol transaction
        P->>P: Validate: !emergency_pause, amount > 0
        P->>S: Fetch BackerDeposit account
        P->>T: Fetch TreasuryPool account
        
        P->>P: Check: deposited_amount >= amount
        P->>P: Check: liquid_balance >= amount
        
        alt Insufficient liquid balance
            P-->>F: Error: "Insufficient liquid balance"
        else Sufficient balance
            P->>P: Calculate new deposited_amount
            P->>P: Update: deposited_amount -= amount
            P->>P: Update: total_deposited -= amount
            P->>P: Update: liquid_balance -= amount
            
            Note over P: Update reward_debt proportionally:<br/>reward_debt = (new_deposited_amount * reward_per_share)
            
            P->>S: Update: reward_debt = new_deposited_amount * reward_per_share
            P->>T: Transfer amount to backer wallet
            P->>P: Emit SolUnstaked event
            P-->>F: Transaction confirmed
            F->>F: Refresh on-chain data
            F-->>B: Show success: "Successfully unstaked X SOL"
        end
    end
```

---

## 5. Admin Operations

### 5.1. Credit Fees to Pool

Flow when admin credits fees to reward pool (after developer payment).

```mermaid
sequenceDiagram
    participant A as Admin (Backend)
    participant BE as Backend (NestJS)
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant R as Reward Pool PDA

    A->>BE: Call creditFeeToPool(feeReward, feePlatform)
    BE->>P: credit_fee_to_pool instruction
    
    Note over BE,P: Admin-only instruction
    
    P->>P: Validate: admin == treasury_pool.admin
    P->>P: Update: reward_pool_balance += fee_reward
    P->>P: Update: platform_pool_balance += fee_platform
    
    alt total_deposited > 0
        P->>P: Update reward_per_share
        Note over P: reward_per_share += (fee_reward * PRECISION) / total_deposited
    end
    
    P->>P: Emit FeeCredited event
    P-->>BE: Fees credited
    BE-->>A: Success
```

### 5.2. Fund Temporary Wallet

Flow when admin funds temporary wallet for deployment.

```mermaid
sequenceDiagram
    participant A as Admin (Backend)
    participant BE as Backend (NestJS)
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant DR as DeployRequest PDA
    participant EK as Ephemeral Wallet

    A->>BE: fundTemporaryWallet(programHash, ephemeralKey, cost)
    BE->>P: fund_temporary_wallet instruction
    
    Note over BE,P: Admin-only instruction<br/>Funds from TreasuryPool.liquid_balance<br/>(NOT RewardPool or PlatformPool)
    
    P->>P: Validate: admin == treasury_pool.admin
    P->>P: Check: liquid_balance >= deployment_cost
    
    alt Insufficient liquid balance
        P-->>BE: Error: "InsufficientLiquidBalance"
    else Sufficient balance
        P->>T: Transfer deployment_cost to ephemeral wallet
        P->>P: Update: liquid_balance -= deployment_cost
        P->>DR: Update: borrowed_amount = deployment_cost
        P->>DR: Update: ephemeral_key = ephemeral_wallet
        P->>P: Emit TemporaryWalletFunded event
        P-->>BE: Temporary wallet funded
        BE-->>A: Success
    end
```

### 5.3. Initialize Treasury Pool

Flow when admin initializes the treasury pool for the first time.

```mermaid
sequenceDiagram
    participant A as Admin (Backend)
    participant BE as Backend (NestJS)
    participant P as Solana Program
    participant T as Treasury Pool PDA
    participant R as Reward Pool PDA
    participant PL as Platform Pool PDA

    A->>BE: initializeTreasuryPool(devWallet)
    BE->>P: initialize instruction
    
    Note over BE,P: Admin-only instruction<br/>Creates all PDAs
    
    P->>T: Initialize TreasuryPool PDA
    P->>R: Initialize Reward Pool PDA
    P->>PL: Initialize Platform Pool PDA
    
    P->>P: Set initial values:
    Note over P: reward_per_share = 0<br/>total_deposited = 0<br/>liquid_balance = 0<br/>reward_pool_balance = 0<br/>platform_pool_balance = 0<br/>admin = admin_pubkey<br/>dev_wallet = dev_wallet<br/>emergency_pause = false
    
    P->>P: Store PDA bumps
    P->>P: Emit TreasuryPoolInitialized event
    P-->>BE: Treasury pool initialized
    BE-->>A: Success
```

---

## Architecture Overview

### Account Relationships

```mermaid
graph TB
    TP[Treasury Pool PDA<br/>State Account]
    RP[Reward Pool PDA<br/>Holds reward fees]
    PP[Platform Pool PDA<br/>Holds platform fees]
    BD[BackerDeposit PDA<br/>Per-backer state]
    DR[DeployRequest PDA<br/>Per-deployment state]
    EK[Ephemeral Wallet<br/>Temporary deployment wallet]
    
    TP -->|tracks| RP
    TP -->|tracks| PP
    TP -->|references| BD
    TP -->|references| DR
    DR -->|funds| EK
    
    style TP fill:#e1f5ff
    style RP fill:#fff4e1
    style PP fill:#ffe1f5
    style BD fill:#e1ffe1
    style DR fill:#f5e1ff
    style EK fill:#ffe1e1
```

### Reward Calculation Model

```mermaid
graph LR
    A[Backer Deposits X SOL] -->|100% to Treasury| B[Treasury Pool]
    C[Developer Pays Fees] -->|1% to Reward Pool| D[Reward Pool]
    C -->|0.1% to Platform Pool| E[Platform Pool]
    
    D -->|Updates| F[reward_per_share]
    F -->|Used for| G[Reward Calculation]
    
    G -->|Formula| H[claimable = deposited_amount * reward_per_share - reward_debt / PRECISION]
    
    style A fill:#e1ffe1
    style C fill:#ffe1f5
    style D fill:#fff4e1
    style E fill:#ffe1e1
    style F fill:#e1f5ff
    style G fill:#f5e1ff
    style H fill:#ffffe1
```

---

## Notes

1. **Reward-Per-Share Model**: The system uses a reward-per-share accumulator pattern for efficient reward distribution without iterating through all backers.

2. **Fee Structure**:
   - Backers: No fees (100% of deposit goes to Treasury)
   - Developers: 
     - monthlyFee (1% monthly) + serviceFee → Reward Pool
     - platformFee (0.1% platform) → Platform Pool

3. **Pool Separation**:
   - **Treasury Pool**: Holds all backer deposits (principal), funds deployments, receives recovered funds
   - **Reward Pool**: Holds monthly fees + service fees (for rewards to backers)
   - **Platform Pool**: Holds platform fees (0.1% developer fees, admin operations)

4. **Deployment Flow**:
   - Payment split: monthlyFee + serviceFee → Reward Pool, platformFee → Platform Pool (off-chain)
   - Backend verifies 2 transfers in one transaction
   - Admin creates DeployRequest (credits fees to pools, updates reward_per_share)
   - Admin funds temporary wallet from TreasuryPool.liquid_balance (NOT RewardPool)
   - Backend deploys program
   - Admin confirms success/failure
   - Recovered funds go back to TreasuryPool.liquid_balance (NOT PlatformPool)

5. **Security**:
   - All admin operations require admin signature
   - Payment verification before creating DeployRequest
   - Liquid balance checks before withdrawals
   - Emergency pause mechanism available

