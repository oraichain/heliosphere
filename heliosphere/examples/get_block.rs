use std::time::Duration;

use actix::System;
use heliosphere::RpcClient;
use heliosphere_core::block::BlockBy;

fn main() {
    let api = "https://api.shasta.trongrid.io";
    let client = RpcClient::new(api, Duration::from_secs(120)).unwrap();
    let runner = System::new();
    runner.block_on(async move {
        let latest_block = client.get_latest_block().await.unwrap();
        println!("block number: {}", latest_block.block_number());
        println!("block id: {}", latest_block.block_id);

        let block_byid = client
            .get_block(BlockBy::Id(latest_block.block_id))
            .await
            .unwrap();
        let block_bynum = client
            .get_block(BlockBy::Number(latest_block.block_number()))
            .await
            .unwrap();
        assert!(
            latest_block == block_byid && block_byid == block_bynum,
            "invalid block"
        );
    });
}
