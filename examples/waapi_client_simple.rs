use serde_json::json;
use std::error::Error;
use wamp_async::WaapiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Create and connect to WAAPI server with simplified API
    println!("Connecting to WAAPI server...");
    let mut client = WaapiClient::builder()
        .host("localhost")
        .port(8080)
        .ssl_verify(false)
        .connect()
        .await?;

    println!("Connected to realm: {}", client.realm());

    // Call WAAPI to get all Event objects
    println!("\nCalling ak.wwise.core.object.get...");
    let result = client
        .call(
            "ak.wwise.core.object.get",
            Some(json!({
                "from": {
                    "ofType": ["Event"]
                }
            })),
            Some(json!({
                "return": ["name", "id", "type", "path"]
            })),
        )
        .await?;

    // Print the result
    println!("\nResult:");
    println!("{}", serde_json::to_string_pretty(&result)?);

    // Extract and display events
    if let Some(events) = result.get("return").and_then(|v| v.as_array()) {
        println!("\nFound {} event(s):", events.len());
        for event in events {
            if let Some(name) = event.get("name").and_then(|v| v.as_str()) {
                println!("  - {}", name);
            }
        }
    }

    // Close the connection gracefully
    println!("\nClosing connection...");
    client.close().await?;
    println!("Done!");

    Ok(())
}
