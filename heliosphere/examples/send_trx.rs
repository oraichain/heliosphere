use std::time::Duration;

use actix::System;
use heliosphere::RpcClient;
use heliosphere_core::Address;
use heliosphere_signer::{keypair::Keypair, signer::Signer};

fn main() {
    let api = "https://api.shasta.trongrid.io";
    let keypair = Keypair::from_hex_key(option_env!("PRIV_KEY").unwrap()).unwrap();
    let client = RpcClient::new(api, Duration::from_secs(120)).unwrap();
    let from = keypair.address();
    let to: Address = "TB9n2jzcWoqta1xX2Mv8P3y9tyUNsGTFsQ".parse().unwrap();
    let amount = 1;

    let runner = System::new();
    runner.block_on(async move {
        let old_balance = client.get_account_balance(&from).await.unwrap();
        let mut tx = client.trx_transfer(&from, &to, amount).await.unwrap();
        keypair.sign_transaction(&mut tx).unwrap();
        let txid = client.broadcast_transaction(&tx).await.unwrap();
        println!("Txid: {}", txid);
        println!("Confirming...");
        let info = client
            .await_confirmation(txid, Duration::from_secs(60))
            .await
            .unwrap();
        println!("{:?}", info);
        let new_balance = client.get_account_balance(&from).await.unwrap();
        assert!(old_balance >= new_balance + amount); // including TRX burn
    });
}
