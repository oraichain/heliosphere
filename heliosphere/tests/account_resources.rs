use std::time::Duration;

use heliosphere::RpcClient;
use heliosphere_signer::{keypair::Keypair, signer::Signer};

#[tokio::test]
async fn test_account_resources() {
    let api = "https://api.shasta.trongrid.io";
    let keypair = Keypair::from_hex_key(option_env!("PRIV_KEY").unwrap()).unwrap();
    let account = keypair.address();
    let client = RpcClient::new(api, Duration::from_secs(120)).unwrap();
    let resources = client.get_account_resources(&account).await.unwrap();
    println!("{:?}", resources);
}
