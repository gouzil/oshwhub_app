use serde_json::{Value, json};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = args
        .iter()
        .find(|arg| !arg.starts_with("--"))
        .map_or(r#"{"command":"health"}"#, String::as_str);
    let output = oshwhub_core::dispatch_json(input).await;
    let value: Value = serde_json::from_str(&output).expect("core always returns JSON");
    if args.iter().any(|arg| arg == "--full") {
        println!("{output}");
    } else if let Some(data) = value.get("data") {
        let summary = if data.get("batches").is_some() {
            json!({"id":data["id"],"name":data["name"],"bounds":data["bounds"],"layers":data["layers"],"batches":data["batches"].as_array().map(Vec::len),"texts":data["texts"].as_array().map(Vec::len),"objects":data["objectCount"],"warnings":data["warnings"],"metrics":data["metrics"]})
        } else if let Some(items) = data["items"].as_array() {
            json!({"page":data["page"],"total":data["total"],"hasMore":data["hasMore"],"items":items.iter().map(|item|json!({"id":item["id"],"name":item["name"]})).collect::<Vec<_>>()})
        } else if data.get("summary").is_some() {
            json!({"summary":data["summary"],"contentBlocks":data["content"].as_array().map(Vec::len),"boards":data["boards"].as_array().map(|boards|boards.iter().map(|board|json!({"id":board["id"],"name":board["name"],"pcbId":board["pcbId"],"bomRows":board["bom"].as_array().map(Vec::len)})).collect::<Vec<_>>())})
        } else {
            data.clone()
        };
        println!(
            "{}",
            json!({"ok":true,"requestId":value["requestId"],"data":summary})
        );
    } else {
        println!("{output}");
    }
    if value["ok"] != true {
        std::process::exit(1);
    }
}
