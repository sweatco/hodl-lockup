#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const os = require('os');
const csv = require('csv-parser');
const yargs = require('yargs/yargs');
const { hideBin } = require('yargs/helpers');
const dotenv = require('dotenv');
const nearAPI = require('near-api-js');
const { connect } = nearAPI;

// Load environment variables
dotenv.config();

// Parse command line arguments
const argv = yargs(hideBin(process.argv))
    .option('issue-date', {
        alias: 'd',
        description: 'Timestamp for issue date',
        type: 'number',
        demandOption: true
    })
    .option('batch-size', {
        alias: 'n',
        description: 'Number of grants to process in each batch',
        type: 'number',
        demandOption: true
    })
    .option('csv', {
        alias: 'c',
        description: 'Path to CSV file containing grants',
        type: 'string',
        default: 'grants.csv'
    })
    .help()
    .alias('help', 'h')
    .argv;

// Validate environment variables
const requiredEnvVars = ['HODL_CONTRACT_ID', 'ISSUER_ACCOUNT_ID', 'NEAR_NETWORK'];
for (const envVar of requiredEnvVars) {
    if (!process.env[envVar]) {
        console.error(`Error: ${envVar} environment variable is required`);
        process.exit(1);
    }
}

const HODL_CONTRACT_ID = process.env.HODL_CONTRACT_ID;
const ISSUER_ACCOUNT_ID = process.env.ISSUER_ACCOUNT_ID;
const NEAR_NETWORK = process.env.NEAR_NETWORK || 'testnet';

// Configure NEAR connection
const configureNear = async () => {
    // Use FileSystemKeyStore for ~/.near-credentials
    const credentialsPath = path.join(os.homedir(), '.near-credentials');

    if (!fs.existsSync(credentialsPath)) {
        console.error(`Error: Credentials directory not found at ${credentialsPath}`);
        console.error(`Please make sure you've logged in with near-cli: near login`);
        process.exit(1);
    }

    console.log(`Using credentials from ${credentialsPath}`);
    const keyStore = new nearAPI.keyStores.UnencryptedFileSystemKeyStore(credentialsPath);

    // Create the connection first
    const config = {
        networkId: NEAR_NETWORK,
        keyStore,
        nodeUrl: `https://rpc.${NEAR_NETWORK}.near.org`,
        walletUrl: `https://wallet.${NEAR_NETWORK}.near.org`,
        helperUrl: `https://helper.${NEAR_NETWORK}.near.org`,
        explorerUrl: `https://explorer.${NEAR_NETWORK}.near.org`,
    };

    // Connect to NEAR
    const nearConnection = await connect(config);

    // Check if we have the key for the issuer account
    try {
        // Try to get the key directly instead of using hasKey
        const accountKeyPair = await keyStore.getKey(NEAR_NETWORK, ISSUER_ACCOUNT_ID);
        if (!accountKeyPair) {
            throw new Error("No key found");
        }
        console.log(`Found key for ${ISSUER_ACCOUNT_ID} in ~/.near-credentials`);
    } catch (error) {
        console.error(`Error: No key found for ${ISSUER_ACCOUNT_ID} on ${NEAR_NETWORK} network.`);
        console.error(`Please make sure you have logged in with near-cli: near login`);
        process.exit(1);
    }

    return nearConnection;
};

// Read grants from CSV file
const readGrantsFromCsv = (csvFilePath) => {
    return new Promise((resolve, reject) => {
        const grants = [];
        fs.createReadStream(csvFilePath)
            .pipe(csv({ headers: false }))
            .on('data', (row) => {
                const values = Object.values(row);
                if (values.length >= 2) {
                    const accountId = values[0].trim();
                    const amount = values[1].trim();
                    grants.push([accountId, amount]);
                }
            })
            .on('end', () => {
                console.log(`Read ${grants.length} grants from CSV file`);
                resolve(grants);
            })
            .on('error', (error) => {
                reject(error);
            });
    });
};

// Process grants in batches
const processGrants = async (near, issueDate, batchSize, grants) => {
    const account = await near.account(ISSUER_ACCOUNT_ID);
    const failedBatches = [];
    const logFileName = `failed_batches.json`;

    while (grants.length > 0) {
        // Pop n entries from grants
        const batch = grants.splice(0, batchSize);
        console.log(`Processing batch of ${batch.length} grants...`);

        // Format the batch for the contract call
        const formattedBatch = batch.map(([accountId, amount]) => [accountId, amount.toString()]);

        try {
            // Call the issue method on the contract
            const result = await account.functionCall({
                contractId: HODL_CONTRACT_ID,
                methodName: 'issue',
                args: {
                    issue_date: issueDate,
                    amounts: formattedBatch
                },
                gas: '300000000000000', // 300 TGas
                attachedDeposit: '0'
            });

            console.log(`Successfully processed batch. Transaction hash: ${result.transaction.hash}`);
        } catch (error) {
            console.error(`Error processing batch: ${error.message}`);

            // Add the failed batch to our collection
            failedBatches.push({
                issue_date: issueDate,
                amounts: formattedBatch,
                error: error.message,
                timestamp: new Date().toISOString()
            });

            // Write all failed batches to a single log file
            fs.writeFileSync(
                path.join(__dirname, logFileName),
                JSON.stringify(failedBatches, null, 2)
            );
            console.log(`Updated failed batches log: ${logFileName}`);
        }
    }

    return failedBatches;
};

// Main function
const main = async () => {
    try {
        const issueDate = argv['issue-date'];
        const batchSize = argv['batch-size'];
        const csvFilePath = path.resolve(process.cwd(), argv.csv);

        console.log(`Starting issuer script with issue date: ${issueDate}, batch size: ${batchSize}`);
        console.log(`Reading grants from: ${csvFilePath}`);

        // Check if CSV file exists
        if (!fs.existsSync(csvFilePath)) {
            console.error(`Error: CSV file not found at ${csvFilePath}`);
            process.exit(1);
        }

        // Read grants from CSV
        const grants = await readGrantsFromCsv(csvFilePath);
        if (grants.length === 0) {
            console.log('No grants found in CSV file. Exiting.');
            process.exit(0);
        }

        // Connect to NEAR
        console.log('Connecting to NEAR network...');
        const near = await configureNear();

        // Process grants
        console.log('Processing grants...');
        const failedBatches = await processGrants(near, issueDate, batchSize, grants);

        // Summary
        console.log('\nExecution summary:');
        if (failedBatches.length === 0) {
            console.log('All batches processed successfully!');
        } else {
            console.log(`${failedBatches.length} batches failed. Check the log files for details.`);
        }
    } catch (error) {
        console.error(`Fatal error: ${error.message}`);
        process.exit(1);
    }
};

// Run the main function
main(); 