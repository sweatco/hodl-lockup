CALLER_ID=pimp.testnet

CLIFF_LAST_SECOND=1713784635
CLIFF_END_TIME=1713784636
LOCKUP_END_TIME=1808392636
AMOUNT=10000000000000000000
ACCOUNT_ID=sweatty.testnet

LOCKUP_CONTRACT_ID=v8.hodl.sweatty.testnet
TOKEN_ACCOUNT_ID=vfinal.token.sweat.testnet

MSG="{\\\"account_id\\\": \\\"$ACCOUNT_ID\\\",\\\"schedule\\\":[{\\\"timestamp\\\":$CLIFF_END_TIME,\\\"balance\\\":\\\"0\\\"},{\\\"timestamp\\\":$LOCKUP_END_TIME,\\\"balance\\\":\\\"$AMOUNT\\\"}], \\\"vesting_schedule\\\": {\\\"Schedule\\\": [{\\\"timestamp\\\":$CLIFF_LAST_SECOND,\\\"balance\\\":\\\"0\\\"},{\\\"timestamp\\\":$CLIFF_END_TIME,\\\"balance\\\":\\\"$AMOUNT\\\"}]}}"

near call $TOKEN_ACCOUNT_ID ft_transfer_call "{\"receiver_id\": \"$LOCKUP_CONTRACT_ID\", \"amount\": \"$AMOUNT\", \"msg\": \"$MSG\"}" --accountId $CALLER_ID --gas 300000000000000 --depositYocto 1
