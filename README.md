# Nozz.tv

Solana on-chain program powering **Nozz.tv** — enabling tokenized creator ownership, trading logic, and core protocol state management.

## Overview

Traditional streaming platforms leave creators with minimal earnings from ads and subscriptions, and payouts are often delayed and gated behind platform policy. Nozz.tv solves this by letting every streamer issue their own token, backed by an on-chain bonding curve, so that:

- **Streamers** earn instantly from trading fees every time their token is bought or sold, instead of waiting on subscription payout cycles.
- **Viewers** become investors, buying tokens in the creators they support and benefiting as that creator's popularity (and token value) grows.
- **Communities** gain utility — staking a creator's token unlocks subscriber status, access, and rewards, turning passive viewers into engaged stakeholders.

This repository contains the Anchor program that implements that protocol.

## Core Modules

| Module         | Description                                                          |
| -------------- | -------------------------------------------------------------------- |
| `constants`    | Global protocol constants (fee rates, seeds, curve parameters, etc.) |
| `error`        | Custom Anchor error definitions                                      |
| `events`       | On-chain events emitted for indexing/analytics                       |
| `instructions` | All instruction handlers (config, launchpad, staking)                |
| `state`        | Account/state struct definitions                                     |
| `utils`        | Shared helper logic (math, validation, etc.)                         |

## Features

### 1. Platform Config

Admin-managed global configuration that governs protocol-wide parameters.

- `initialize_config` — Initializes the global platform config (admin only).
- `update_config` — Updates the global platform config (admin only).

### 2. Token LaunchPad (Creator Tokenization)

Each streamer can launch a unique token on a bonding curve, reflecting their popularity and market demand. Trading fees are paid directly to the streamer, giving them instant, transparent earnings instead of relying on opaque platform payouts.

- `create_token` — Creates a new streamer token with an associated bonding curve.
- `buy_token` — Buys a streamer token along the bonding curve, with slippage protection via `min_tokens_out`.
- `sell_token` — Sells a streamer token along the bonding curve, with slippage protection via `min_sol_out`.
- `claim_fees` — Allows a streamer to claim accumulated fees from trading activity on their token.
- `graduate_to_dex` — Permissionlessly migrates a token to a DEX once its bonding curve has been completed, unlocking deeper liquidity.

### 3. Stake-to-Subscribe

Viewers can stake a creator's token to unlock subscriber status, access perks, and earn staking rewards — turning token ownership into ongoing engagement and loyalty.

- `stake` — Stakes creator tokens to earn rewards and gain subscriber status.
- `unstake` — Unstakes tokens; subscriber status is revoked immediately if the remaining stake falls below the required threshold.
- `claim_stake_rewards` — Claims accumulated staking rewards.
- `update_min_stake` — Lets a creator update the minimum stake amount required for subscriber status.

## Architecture

```
programs/nozz-launchpad/
└── src/
    ├── instructions/
    │   ├── admin/               # initialize_config, update_config
    │   ├── creator/             # claim_fees, create_token, update_min_stake
    │   ├── user/                # buy, sell, graduate, stake, unstake, claim_stake_rewards
    │   └── mod.rs
    ├── state/                   # Account state definitions
    │   └── mod.rs
    ├── constants.rs             # Global constants and PDA seeds
    ├── error.rs                 # Custom error codes
    ├── events.rs                # Emitted on-chain events
    ├── lib.rs                   # Program entrypoint
    └── utils.rs                 # Shared helper functions
```

## Tech Stack

- **Solana** — high-throughput, low-fee L1 for on-chain trading and state.
- **Anchor** — Rust framework for Solana program development.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation)

### Build

```bash
anchor build
```

### Test

```bash
anchor test
```

### Deploy

```bash
anchor deploy
```

## License

MIT

## Contact

- Email: porwalarjun95@gmail.com
- Portfolio: https://arjunporwal.vercel.app/
