# kelly-calc

A terminal UI for calculating optimal position sizes using the **Kelly Criterion**.  
Built with Rust + [Ratatui](https://github.com/ratatui-org/ratatui).

```
⬡ KELLY CRITERION  ─  Position Sizing Calculator
┌─ Inputs ──────────────────┐┌─ Results ──────────────────────────────┐
│ ▶ Balance                 ││                                        │
│    $ 10000█               ││  Full Kelly              18.33%        │
│   Win Rate                ││  Frac Kelly               9.17%        │
│    % 55                   ││  Applied                  5.00%        │
│   Win/Loss Ratio          ││                                        │
│    x 1.5                  ││  ████████░░░░░░░░░░░░░░░░░░░░░░░░░    │
│   Kelly Fraction          ││  Risk: 5.00% of capital                │
│    % 50                   ││                                        │
│   Max Risk/Trade          ││  ─── Amounts ──────────────────        │
│    % 5                    ││  Balance                $10000.00      │
│                           ││  Position Size            $500.00      │
│                           ││  Max Loss                 $500.00      │
│                           ││  Expected Gain            $162.50      │
│                           ││                                        │
│                           ││  ─── Statistics ───────────────        │
│                           ││  Edge                      8.33%       │
│                           ││  Ruin Risk               0.0000%       │
└───────────────────────────┘└────────────────────────────────────────┘
 ↑↓/Tab  navigate   0-9/.  edit   ?  help   q  quit
```

## What is the Kelly Criterion?

The Kelly Criterion is a formula that tells you what fraction of your capital to
risk on a trade to maximize long-term growth:

```
f* = (b·p − q) / b
```

| Variable | Meaning |
|----------|---------|
| `p` | Probability of winning (win rate) |
| `q` | Probability of losing `(1 − p)` |
| `b` | Win/loss ratio (avg winner ÷ avg loser) |
| `f*` | Optimal fraction of capital to risk |

If the result is **negative or zero**, the setup has no statistical edge and
Kelly recommends **no position**.

## Parameters

| Field | Description |
|-------|-------------|
| **Balance** | Your total trading capital in any currency. |
| **Win Rate** | Percentage of your trades that close profitable (0–100). |
| **Win/Loss Ratio** | Average winning trade size divided by average losing trade size. A ratio of 1.5 means your winners are 50% larger than your losers on average. |
| **Kelly Fraction** | Scales down the full Kelly recommendation. `100` = Full Kelly (aggressive), `50` = Half-Kelly (recommended for most traders), `25` = Quarter-Kelly (conservative). |
| **Max Risk/Trade** | A hard cap on the percentage of balance you're willing to risk per trade, regardless of what Kelly suggests. Acts as a safety net when Kelly outputs a large number. |

## Output

| Result | Meaning |
|--------|---------|
| **Full Kelly** | Raw Kelly recommendation — maximum growth fraction. |
| **Frac Kelly** | Kelly × your chosen fraction (e.g. Half-Kelly). |
| **Applied** | Final percentage used after capping at Max Risk. |
| **Position Size** | Dollar amount to put into the trade. |
| **Max Loss** | Maximum capital at risk (assumes full position is lost). |
| **Expected Gain** | Statistically expected profit per trade at this size. |
| **Edge** | `b·p − q` — your statistical advantage per unit risked. |
| **Ruin Risk** | Approximate probability of going broke given current parameters. |

## Install

**Requirements:** Rust stable (1.70+)

```bash
git clone <repo>
cd kelly-calc
cargo build --release
./target/release/kelly
```

Or run directly:

```bash
cargo run
```

### NixOS / nix-shell

If you're on NixOS, a `flake.nix` with the required build inputs:

```nix
{
  description = "kelly-calc";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust = pkgs.rust-bin.stable.latest.default;
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [ rust pkg-config ];
        };
      }
    );
}
```

## Keybindings

| Key | Action |
|-----|--------|
| `↑` / `↓` / `Tab` | Navigate between fields |
| `0–9` / `.` | Edit the selected field |
| `Backspace` | Delete last character |
| `?` | Toggle help popup |
| `q` / `Ctrl-C` | Quit |

## Dependencies

```toml
ratatui   = "0.29"
crossterm = "0.28"
```

## License

MIT