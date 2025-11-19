# D2D Program Sol - Instructions Structure

## 📁 Cấu trúc Instructions theo Role

```
src/instructions/
├── mod.rs                 # Main module exports
├── initialize.rs          # Program initialization
├── lender/               # Lender operations
│   ├── mod.rs
│   ├── stake_sol.rs      # Stake SOL into treasury
│   ├── unstake_sol.rs    # Unstake SOL from treasury
│   └── claim_rewards.rs  # Claim accumulated rewards
├── deploy_program.rs     # Deploy program with 2 signatures
├── developer/            # Developer operations
│   ├── mod.rs
│   └── pay_subscription.rs # Pay monthly subscription
└── admin/               # Admin operations
    ├── mod.rs
    ├── distribute_rewards.rs  # Distribute rewards to lenders
    ├── update_apy.rs          # Update APY rate
    ├── suspend_expired_programs.rs # Suspend expired programs
    └── emergency_pause.rs      # Emergency pause/unpause
```

## 🎯 Role-based Instructions

### 🏦 Lender Instructions
- **`stake_sol`** - Stake SOL vào treasury pool để kiếm lợi nhuận
- **`unstake_sol`** - Unstake SOL từ treasury (có thể có lock period)
- **`claim_rewards`** - Claim rewards đã tích lũy từ APY

### 👨‍💻 Developer Instructions
- **`pay_subscription`** - Trả phí hàng tháng để duy trì quyền sử dụng program

### 🔧 Admin Instructions
- **`distribute_rewards`** - Phân phối rewards cho lender
- **`update_apy`** - Cập nhật APY rate
- **`suspend_expired_programs`** - Suspend các program hết hạn subscription
- **`emergency_pause`** - Emergency pause/unpause toàn bộ system
- **`confirm_deployment_success`** - Xác nhận deployment thành công
- **`confirm_deployment_failure`** - Xác nhận deployment thất bại và refund

### 🤝 Shared Instructions
- **`deploy_program`** - Deploy program với cả developer và admin signatures

## 🔄 Workflow

1. **Initialization**: Admin khởi tạo treasury pool với APY ban đầu
2. **Lender Staking**: Lender stake SOL để cung cấp liquidity
3. **Program Deployment**: Developer và Admin cùng ký để deploy program
   - Developer trả phí vào treasury
   - Admin chuyển deployment cost từ treasury → ephemeral key
   - Status = `PendingDeployment`
4. **Deployment Confirmation**: Admin xác nhận deployment thành công/thất bại
   - **Success**: Status = `Active`, program được deploy
   - **Failure**: Refund developer, return deployment cost to treasury
5. **Subscription**: Developer trả phí hàng tháng để duy trì quyền sử dụng
6. **Rewards**: Admin phân phối rewards cho lender từ developer fees

## 💡 Lợi ích của cấu trúc này

- **Dễ đọc**: Mỗi role có module riêng
- **Dễ maintain**: Thay đổi logic của một role không ảnh hưởng role khác
- **Scalable**: Dễ dàng thêm instructions mới cho từng role
- **Clear separation**: Phân chia rõ ràng trách nhiệm của từng role
