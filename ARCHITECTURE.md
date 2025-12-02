# D2D Program Architecture - Reward System

## 📋 Tổng quan luồng dự án

### 1. **Staking Flow (Backer Stake SOL)**
```
User → stake_sol() → Treasury Pool PDA
  ├─ Calculate fees: 1% reward fee + 0.1% platform fee
  ├─ Transfer net deposit to Treasury Pool PDA
  ├─ Transfer fees to Reward Pool PDA + Platform Pool PDA
  ├─ Update total_deposited
  └─ Create/Update BackerDeposit account
     ├─ deposited_amount = net deposit (after fees)
     ├─ reward_debt = deposited_amount * reward_per_share (at deposit time)
     └─ is_active = true
```

### 2. **Fee Collection Flow (Developer Pays)**
```
Developer → Pay fees → Reward Pool PDA + Platform Pool PDA
  ├─ Monthly fee (1%) → Reward Pool
  ├─ Platform fee (0.1%) → Platform Pool
  └─ Backend calls credit_fee_to_pool()
     ├─ Update reward_pool_balance (in TreasuryPool struct)
     ├─ Update platform_pool_balance
     └─ Update reward_per_share:
        reward_per_share += (fee_reward * PRECISION) / total_deposited
```

### 3. **Reward Calculation (Reward-Per-Share Model)**
```
For each BackerDeposit:
  accumulated = deposited_amount * reward_per_share
  claimable = (accumulated - reward_debt) / PRECISION
  
Where:
  - reward_per_share: Accumulator that increases when fees are credited
  - reward_debt: Snapshot of accumulated at deposit time
  - PRECISION: 1e12 (to maintain precision in u128 calculations)
```

### 4. **Claim Rewards Flow**
```
Backer → claim_rewards() → Reward Pool PDA
  ├─ Calculate claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
  ├─ Verify reward_pool_balance >= claimable
  ├─ Transfer from Reward Pool PDA → Backer
  ├─ Update claimed_total += claimable
  ├─ Update reward_debt = deposited_amount * reward_per_share (reset to current)
  └─ Debit reward_pool_balance -= claimable
```

### 5. **Deployment Flow**
```
Developer → Pay fees → Create deploy request
  ├─ Payment split: Reward Pool (1%) + Platform Pool (0.1%)
  ├─ Backend verifies payment
  ├─ Backend calls create_deploy_request()
  ├─ Backend deploys program
  └─ Backend calls confirm_deployment_success()
     └─ Fees already in pools, reward_per_share updated
```

## 🏗️ Kiến trúc hiện tại

### **State Accounts:**

1. **TreasuryPool** (PDA: `treasury_pool`)
   - `reward_per_share: u128` - Accumulator for rewards
   - `total_deposited: u64` - Total SOL staked by all backers
   - `liquid_balance: u64` - Available for withdrawals
   - `reward_pool_balance: u64` - Tracked balance in Reward Pool
   - `platform_pool_balance: u64` - Tracked balance in Platform Pool

2. **Reward Pool PDA** (`reward_pool`)
   - Holds actual SOL from fees (1% monthly fees)
   - Program-owned account
   - Used for reward distribution

3. **Platform Pool PDA** (`platform_pool`)
   - Holds actual SOL from platform fees (0.1%)
   - Program-owned account
   - Can be withdrawn by admin

4. **BackerDeposit** (PDA: `lender_stake` + backer pubkey)
   - `backer: Pubkey` - Backer wallet
   - `deposited_amount: u64` - Amount staked (net after fees)
   - `reward_debt: u128` - Snapshot at deposit time
   - `claimed_total: u64` - Total rewards claimed
   - `is_active: bool` - Is deposit active

## 🔧 Vấn đề hiện tại

1. **Admin không thể rút từ Reward Pool**
   - Chỉ có `admin_withdraw` cho Platform Pool (Admin Pool)
   - Reward Pool chỉ có thể được claim bởi backers

2. **Tracking không chính xác**
   - `reward_pool_balance` trong struct có thể out of sync với actual balance
   - Leaderboard không hiển thị đúng vì không fetch được accounts

3. **Leaderboard không hoạt động**
   - Không fetch được BackerDeposit accounts
   - Filter quá strict (chỉ hiển thị active với deposits)

## ✅ Giải pháp đề xuất

### 1. **Thêm Admin Withdraw từ Reward Pool**
- Tạo instruction `admin_withdraw_reward_pool`
- Admin có thể rút SOL từ Reward Pool (với lý do)
- Cập nhật `reward_pool_balance` trong struct
- Emit event để audit

### 2. **Cải thiện Tracking**
- Đảm bảo `reward_pool_balance` luôn sync với actual balance
- Thêm method `sync_reward_pool_balance()` tương tự `sync_liquid_balance()`
- Log chi tiết khi credit/debit rewards

### 3. **Fix Leaderboard**
- Sửa logic fetch accounts (đã làm)
- Nới lỏng filter: hiển thị cả accounts có rewards (claimable hoặc claimed)
- Log chi tiết để debug

### 4. **Kiến trúc mới**

```
┌─────────────────────────────────────────────────────────┐
│                    Treasury Pool PDA                     │
│  - reward_per_share: u128                                │
│  - total_deposited: u64                                  │
│  - reward_pool_balance: u64 (tracked)                   │
│  - platform_pool_balance: u64 (tracked)                  │
└─────────────────────────────────────────────────────────┘
           │                    │
           │                    │
    ┌──────▼──────┐      ┌──────▼──────┐
    │ Reward Pool │      │Platform Pool│
    │    PDA      │      │    PDA      │
    │ (1% fees)   │      │ (0.1% fees) │
    └─────────────┘      └─────────────┘
           │                    │
           │                    │
    ┌──────▼──────┐      ┌──────▼──────┐
    │  Backers    │      │   Admin     │
    │  Claim      │      │  Withdraw   │
    └─────────────┘      └─────────────┘

┌─────────────────────────────────────────────────────────┐
│              BackerDeposit Accounts (PDA)               │
│  For each backer:                                       │
│  - deposited_amount: u64                                │
│  - reward_debt: u128                                    │
│  - claimed_total: u64                                   │
│  - is_active: bool                                      │
└─────────────────────────────────────────────────────────┘
```

## 📊 Reward Calculation Formula

```
reward_per_share += (fee_reward * PRECISION) / total_deposited

For each backer:
  accumulated = deposited_amount * reward_per_share
  claimable = (accumulated - reward_debt) / PRECISION
  total_rewards = claimable + claimed_total
```

## 🔐 Security Considerations

1. **Admin Withdraw từ Reward Pool:**
   - Chỉ admin có thể withdraw
   - Phải có reason (audit trail)
   - Emit event để tracking
   - Không được withdraw quá reward_pool_balance

2. **Reward Distribution:**
   - Verify reward_pool_balance >= claimable trước khi claim
   - Update reward_debt sau khi claim để tránh double claim
   - Sync balance thường xuyên

3. **Leaderboard:**
   - Fetch từ on-chain (không trust backend)
   - Verify calculations match on-chain state
   - Log tất cả để debug

