use std::time::Duration;

use ethabi::{ethereum_types::U256, ParamType, Token};
use heliosphere::{MethodCall, RpcClient};
use heliosphere_core::Address;
use heliosphere_signer::{keypair::Keypair, signer::Signer};

#[tokio::test]
async fn test_send_token() {
    let api = "https://api.shasta.trongrid.io";
    let keypair = Keypair::from_hex_key(option_env!("PRIV_KEY").unwrap()).unwrap();
    let client = RpcClient::new(api, Duration::from_secs(120)).unwrap();
    let from = keypair.address();
    let to: Address = "TB9n2jzcWoqta1xX2Mv8P3y9tyUNsGTFsQ".parse().unwrap();
    let usdt: Address = "TG3XXyExBkPp9nzdajDZsozEu4BkaSJozs".parse().unwrap();
    let amount: u64 = 1; // 0.000001 USDT

    // Fetch account balance before
    let method_call_balance = MethodCall {
        caller: &from,
        contract: &usdt,
        selector: "balanceOf(address)",
        parameter: &ethabi::encode(&[Token::Address(from.into())]),
    };
    let res = &ethabi::decode(
        &[ParamType::Uint(256)],
        &client
            .query_contract(&method_call_balance)
            .await
            .unwrap()
            .constant_result(0)
            .unwrap(),
    )
    .unwrap()[0];
    let old_balance = match res {
        Token::Uint(x) => x,
        _ => panic!("Wrong type"),
    };

    let method_call = MethodCall {
        caller: &from,
        contract: &usdt,
        selector: "transfer(address,uint256)",
        parameter: &ethabi::encode(&[Token::Address(to.into()), Token::Uint(U256::from(amount))]),
    };
    // Estimate energy usage
    let estimated = client.estimate_energy(&method_call).await.unwrap();
    println!("Estimated energy usage: {}", estimated);

    // Send token
    let mut tx = client
        .trigger_contract(&method_call, 0, None)
        .await
        .unwrap();
    keypair.sign_transaction(&mut tx).unwrap();
    let txid = client.broadcast_transaction(&tx).await.unwrap();
    println!("Txid: {}", txid);
    println!("Confirming...");
    client
        .await_confirmation(txid, Duration::from_secs(60))
        .await
        .unwrap();

    // Fetch account balance after
    let res = &ethabi::decode(
        &[ParamType::Uint(256)],
        &client
            .query_contract(&method_call_balance)
            .await
            .unwrap()
            .constant_result(0)
            .unwrap(),
    )
    .unwrap()[0];
    let new_balance = match res {
        Token::Uint(x) => x,
        _ => panic!("Wrong type"),
    };
    assert_eq!(*old_balance, *new_balance + amount);
}
