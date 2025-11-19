# D2D Program Sol - Decentralized Deployment Platform for Solana

A decentralized deployment platform that enables developers to deploy Solana programs on mainnet at the lowest cost through a staking/lending mechanism. Think Vercel for Solana blockchain.

## 🎯 Overview

D2D Program Sol connects three key actors in a sustainable DeFi ecosystem:

- **Lenders**: Stake SOL to provide liquidity and earn APY rewards from developer fees
- **Developers**: Deploy programs by paying service fees and monthly subscriptions
- **Admin**: Manages deployments, confirms success/failure, and ensures system security

## 🏗️ Architecture

### Core State Accounts

#### **TreasuryPool**
Central pool managing all staked funds and rewards:
- `total_staked`: Total SOL staked by lenders
- `current_apy`: Current APY rate (basis points, max 100%)
- `total_fees_collected`: Total fees from developers
- `total_rewards_distributed`: Total rewards paid to lenders
- `treasury_wallet`: Validated treasury wallet address
- `emergency_pause`: Emergency pause mechanism

#### **LenderStake**
Individual lender staking information:
- `staked_amount`: Amount of SOL staked
- `lock_period`: Optional lock period (0 = flexible)
- `reward_debt`: For compound interest calculation
- `last_claim_time`: Last reward claim timestamp
- `total_claimed`: Total rewards claimed
- `is_active`: Stake status

#### **DeployRequest**
Deployment request tracking:
- `service_fee`: One-time deployment fee
- `monthly_fee`: Monthly subscription fee
- `deployment_cost`: Actual deployment cost from treasury
- `subscription_paid_until`: Subscription validity period
- `ephemeral_key`: Temporary key for deployment
- `deployed_program_id`: Deployed program ID (set after success)
- `status`: Current deployment status

## 🔄 Business Flow

### 1. **Initialization**
```rust
initialize(initial_apy: u64, treasury_wallet: Pubkey)
```
- Admin initializes treasury pool
- Sets initial APY rate (max 100%)
- Configures treasury wallet

### 2. **Lender Operations**

#### **Stake SOL**
```rust
stake_sol(amount: u64, lock_period: i64)
```
- Stake SOL into treasury
- Optional lock period for higher rewards
- Auto-compound existing rewards
- Provides liquidity for deployments

#### **Unstake SOL**
```rust
unstake_sol(amount: u64)
```
- Unstake SOL (if not locked)
- Auto-claim rewards before unstaking
- Deactivates stake if fully unstaked

#### **Claim Rewards**
```rust
claim_rewards()
```
- Claim accumulated APY rewards
- High-precision calculation with overflow protection
- Time-based rewards calculation

### 3. **Developer Operations**

#### **Deploy Program**
```rust
deploy_program(
    program_hash: [u8; 32],
    service_fee: u64,
    monthly_fee: u64,
    initial_months: u32,
    deployment_cost: u64
)
```
- **Requires both Developer + Admin signatures**
- Developer pays service fee + initial subscription
- Admin confirms deployment cost and transfers from treasury
- Status = `PendingDeployment` initially

#### **Pay Subscription**
```rust
pay_subscription(request_id: [u8; 32], months: u32)
```
- Monthly subscription payments
- Extends program usage rights
- Only works for `Active` or `SubscriptionExpired` programs

### 4. **Admin Operations**

#### **Confirm Deployment Success**
```rust
confirm_deployment_success(request_id: [u8; 32], deployed_program_id: Pubkey)
```
- Admin confirms successful deployment
- Sets status = `Active`
- Records deployed program ID

#### **Confirm Deployment Failure**
```rust
confirm_deployment_failure(request_id: [u8; 32], failure_reason: String)
```
- Admin confirms failed deployment
- Sets status = `Failed`
- **Full refund** to developer
- Returns deployment cost to treasury

#### **System Management**
```rust
update_apy(new_apy: u64)           // Update APY rate (max 100%)
suspend_expired_programs()         // Suspend expired programs
emergency_pause(pause: bool)       // Emergency pause/unpause
```

## 🔒 Security Features

### **Multi-Signature Deployment**
- `deploy_program` requires **both Developer + Admin signatures**
- Admin controls deployment cost and ephemeral key
- Developer controls payment and program hash
- Prevents unauthorized deployments

### **Deployment Failure Protection**
- **Full refund** if deployment fails
- Deployment cost returned to treasury
- No financial loss for developers
- Admin confirms success/failure status

### **Treasury Wallet Validation**
All instructions validate treasury wallet:
```rust
constraint = treasury_wallet.key() == treasury_pool.treasury_wallet @ ErrorCode::InvalidTreasuryWallet
```

### **Overflow Protection**
- **High-precision calculations** using u128 arithmetic
- **Checked arithmetic** for all operations
- **Time limits** to prevent calculation overflow
- **APY validation** (max 100%)

### **Role-based Access Control**
- **Lenders**: Can only manage their own stakes
- **Developers**: Can only manage their own deployments
- **Admin**: Full system control with signature validation

## 📊 Deployment Status Flow

```
PendingDeployment → [Admin Deploys] → Active ✅
                 ↘ [Deployment Fails] → Failed ❌ (Full Refund)

Active → [Subscription Expires] → SubscriptionExpired
      → [Non-payment] → Suspended
      → [Developer Cancels] → Cancelled
```

## 🛡️ Calculation Safety

### **Rewards Calculation**
```rust
// High-precision formula with overflow protection
let numerator = staked_amount_u128
    .checked_mul(apy_u128)
    .checked_mul(time_elapsed_u128)
    .checked_mul(1e18); // Precision multiplier

let denominator = 10000 * 86400 * 365; // Annual calculation
let reward = numerator / denominator / 1e18;
```

### **Safety Features**
- ✅ **u128 arithmetic** for intermediate calculations
- ✅ **Checked operations** to prevent overflow
- ✅ **Time limits** (max 1 year elapsed)
- ✅ **APY limits** (max 100%)
- ✅ **Precision multiplier** (1e18) for accuracy

## 💰 Revenue Model

### **For Lenders**
- Earn APY on staked SOL (calculated with high precision)
- Flexible or locked staking options
- Compound interest rewards
- Rewards sourced from developer fees

### **For Developers**
- Pay service fee for deployment
- Pay monthly subscription for program usage
- Access to mainnet deployment at low cost
- **Full refund** if deployment fails

### **For Platform**
- Revenue from developer fees
- Sustainable DeFi model
- Scalable with more users
- No financial risk for developers

## 🚀 Key Features

- **Low-cost Deployment**: Use pooled funds instead of individual deployment costs
- **Subscription Model**: Pay-per-use with monthly subscriptions
- **DeFi Integration**: Earn rewards by providing liquidity
- **Security First**: Multi-signature validation and treasury protection
- **Failure Protection**: Full refunds for failed deployments
- **High Precision**: DEX-level calculation accuracy
- **Scalable**: More lenders = more liquidity = more developers

## 📁 Project Structure

```
src/
├── lib.rs                 # Main program entry point
├── states/               # Account state definitions
│   ├── treasury_pool.rs  # Treasury management
│   ├── lender_stake.rs   # Lender staking
│   ├── deploy_request.rs # Deployment requests
│   └── user_deploy_stats.rs # User statistics
├── instructions/         # Program instructions
│   ├── initialize.rs     # Program initialization
│   ├── deploy_program.rs # Combined deployment
│   ├── lender/          # Lender operations
│   │   ├── stake_sol.rs
│   │   ├── unstake_sol.rs
│   │   └── claim_rewards.rs
│   ├── developer/       # Developer operations
│   │   └── pay_subscription.rs
│   └── admin/           # Admin operations
│       ├── confirm_deployment.rs
│       ├── update_apy.rs
│       ├── suspend_expired_programs.rs
│       └── emergency_pause.rs
├── events.rs            # Event definitions
└── errors.rs           # Error codes
```

## 🎯 Use Cases

1. **Individual Developers**: Deploy personal projects at low cost
2. **Startups**: Access mainnet without large upfront costs
3. **Lenders**: Earn passive income from staked SOL
4. **DeFi Protocols**: Deploy new protocols using pooled resources

## 🔧 Getting Started

### Prerequisites
- Solana CLI tools
- Anchor framework
- Rust toolchain

### Deployment
1. Initialize treasury pool with admin
2. Set initial APY rate (max 100%)
3. Configure treasury wallet
4. Start accepting lender stakes
5. Enable developer deployments

## 📊 Program ID

```
5aai4VhRLDCFP2WSHUbGsiSuZxkWzQahhsRkqdfF2jRh
```

## 🤝 Contributing

This is a decentralized deployment platform designed to make Solana development more accessible and cost-effective. Contributions are welcome to improve security, efficiency, and user experience.

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.