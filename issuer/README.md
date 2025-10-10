# HODL Contract Issuer Script

This script interacts with a HODL smart contract on the NEAR blockchain to issue grants to accounts.

## Prerequisites

- Node.js (v14 or later)
- npm or yarn
- A NEAR account with permissions to call the `issue` method on the HODL contract
- NEAR CLI (for authentication)

## Installation

1. Clone this repository
2. Navigate to the `issuer` directory
3. Install dependencies:

```bash
npm install
```

## Authentication

The script supports two methods for retrieving credentials:

1. **~/.near-credentials/ directory**: Created when you use NEAR CLI's `near login` command
2. **System keychain**: 
   - macOS: Keychain Access
   - Windows: Credential Manager
   - Linux: libsecret

The script will check both locations and use the first valid credential it finds.

### Using NEAR CLI

To add credentials using NEAR CLI:

```bash
near login
```

This will create the necessary credentials in `~/.near-credentials/`.

### Using System Keychain

On macOS, you can also store your NEAR credentials in the Keychain Access application. The script will automatically check the keychain for credentials.

## Configuration

Create a `.env` file in the `issuer` directory with the following variables:

```
HODL_CONTRACT_ID=hodl.near
ISSUER_ACCOUNT_ID=issuer.near
NEAR_NETWORK=testnet
```

- `HODL_CONTRACT_ID`: The account ID where the HODL smart contract is deployed
- `ISSUER_ACCOUNT_ID`: The account ID that is authorized to issue grants
- `NEAR_NETWORK`: The NEAR network to connect to (mainnet, testnet, etc.)

You can copy the `.env.example` file and modify it:

```bash
cp .env.example .env
```

## CSV Format

Create a CSV file with the following format:

```
account1.near,1000000000000000000000000
account2.near,2000000000000000000000000
account3.near,3000000000000000000000000
```

Each line contains an account ID and an amount (as a u128 integer).

## Usage

Run the script with the following command:

```bash
node issuer.js --issue-date <timestamp> --batch-size <n> --csv <path-to-csv>
```

Arguments:

- `--issue-date`, `-d`: Timestamp for issue date (required)
- `--batch-size`, `-n`: Number of grants to process in each batch (required)
- `--csv`, `-c`: Path to CSV file containing grants (default: 'grants.csv')

Example:

```bash
node issuer.js --issue-date 1672531200000 --batch-size 10 --csv ./sample-grants.csv
```

## Error Handling

If errors occur during processing, all failed batches will be written to a single log file in the format `failed_batches_<timestamp>.json`. This file is updated in real-time as failures occur, so you can monitor progress even during a long-running operation. Each entry in the log file includes the issue date, the grant amounts, the error message, and the timestamp of when the error occurred.

## License

MIT 