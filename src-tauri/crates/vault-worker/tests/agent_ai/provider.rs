//! Loopback provider recording the exact OpenAI-compatible HTTP requests. Successful responses
//! exercise the real transport for both embedding and a two-turn tool-using assistant.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
pub struct Provider {
    pub url: String,
    pub requests: Arc<Mutex<Vec<(String, Value)>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Provider {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                if let Ok((socket, _)) = listener.accept() {
                    serve(socket, &log);
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        });
        Self { url, requests, stop, thread: Some(thread) }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn serve(mut socket: TcpStream, log: &Mutex<Vec<(String, Value)>>) {
    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut input = BufReader::new(&socket);
    let mut request = String::new();
    input.read_line(&mut request).unwrap();
    let path = request.split_whitespace().nth(1).unwrap().to_string();
    let mut len = 0;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).unwrap() == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            len = v.trim().parse().unwrap();
        }
    }
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes).unwrap();
    let q: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    log.lock().unwrap().push((path.clone(), q.clone()));
    let (mime, body) = if path.ends_with("/embeddings") {
        let inputs = q["input"].as_array().unwrap();
        let data=inputs.iter().enumerate().map(|(i,v)|json!({"object":"embedding","index":i,"embedding":if v.as_str().unwrap().contains("kitten"){[1.,0.,0.]}else{[0.,1.,0.]}})).collect::<Vec<_>>();
        ("application/json",json!({"object":"list","model":"fixture","data":data,"usage":{"prompt_tokens":1,"total_tokens":1}}).to_string())
    } else if path.ends_with("/chat/completions") {
        let tools = q["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["function"]["name"] == "search_agent_conversations");
        let called = q["messages"].as_array().into_iter().flatten().any(|m| m["role"] == "tool");
        let (delta, finish) = if tools && !called {
            (
                json!({"role":"assistant","tool_calls":[{"index":0,"type":"function","id":"lookup","function":{"name":"search_agent_conversations","arguments":"{\"query\":\"kitten\",\"limit\":1}"}}]}),
                "tool_calls",
            )
        } else {
            (json!({"role":"assistant","content":"Finished."}), "stop")
        };
        let chunk = |delta: Value, reason: Value| json!({"id":"chatcmpl-fixture","object":"chat.completion.chunk","created":1,"model":"fixture","choices":[{"index":0,"delta":delta,"finish_reason":reason}]});
        (
            "text/event-stream",
            format!(
                "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                chunk(delta, Value::Null),
                chunk(json!({}), json!(finish))
            ),
        )
    } else {
        ("application/json", json!({"data":[{"id":"fixture","object":"model"}]}).to_string())
    };
    let _ = write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
