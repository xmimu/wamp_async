use std::error::Error;
use serde_json::json;
use wamp_async::{
    try_into_kwargs, try_into_wamp_dict, Client, ClientConfig, ClientRole, SerializerType,
};


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to the server
    let (mut client, (evt_loop, _rpc_evt_queue)) = Client::connect(
        "ws://localhost:8080/waapi",
        Some(
            ClientConfig::default()
                .set_ssl_verify(false)
                // Restrict our roles
                .set_roles(vec![ClientRole::Caller])
                // Only use Json serialization
                .set_serializers(vec![SerializerType::Json]),
        ),
    )
    .await?;
    println!("Connected !!");

    // Spawn the event loop
    tokio::spawn(evt_loop);

    println!("Joining realm");
    client.join_realm("realm1").await?;

    let send_kwargs = try_into_kwargs(json!({
        "from": {
            "ofType": ["Event"]
        },
    }))?;
    let options = try_into_wamp_dict(json!({
            "return": ["name", "id", "type", "path"]
    }))?;

    match client
        .call(
            "ak.wwise.core.object.get",
            None,
            Some(send_kwargs),
            Some(options),
        )
        .await
    {
        Ok((res_args, res_kwargs)) => {
            println!("\tGot {:?} {:?}", res_args, res_kwargs);
        }
        Err(e) => {
            println!("Error calling ({:?})", e);
        }
    };

    println!("Leaving realm");
    client.leave_realm().await?;

    client.disconnect().await;
    Ok(())
}
