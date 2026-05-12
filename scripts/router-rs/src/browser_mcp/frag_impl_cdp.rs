// `impl CdpClient`。
impl CdpClient {
    fn connect(port: u16) -> Result<Self, Value> {
        let websocket_url = cdp_version_json(port)?
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                browser_error(
                    "CDP_CONNECT_FAILED",
                    "Chrome did not expose a browser websocket URL.",
                    &["retry browser_open"],
                    true,
                )
            })?;
        let (socket, _) = connect(websocket_url.as_str()).map_err(|err| {
            browser_error(
                "CDP_CONNECT_FAILED",
                &format!("connect Chrome CDP websocket failed: {err}"),
                &["retry browser_open"],
                true,
            )
        })?;
        Ok(Self {
            _port: port,
            next_id: 0,
            socket,
        })
    }

    fn call(
        &mut self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = Map::new();
        message.insert("id".to_string(), json!(id));
        message.insert("method".to_string(), Value::String(method.to_string()));
        message.insert("params".to_string(), params);
        if let Some(session_id) = session_id {
            message.insert(
                "sessionId".to_string(),
                Value::String(session_id.to_string()),
            );
        }
        self.socket
            .send(Message::Text(Value::Object(message).to_string()))
            .map_err(|err| {
                browser_error(
                    "CDP_CALL_FAILED",
                    &format!("{method} send failed: {err}"),
                    &["retry after refreshing browser state"],
                    true,
                )
            })?;
        self.set_read_timeout(CDP_RECV_TIMEOUT)?;
        loop {
            let event = self.read_message()?;
            if event.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = event.get("error") {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("{method} failed: {error}"),
                        &["retry after refreshing browser state"],
                        true,
                    ));
                }
                return Ok(event.get("result").cloned().unwrap_or_else(|| json!({})));
            }
        }
    }

    fn drain_events(&mut self, timeout: Duration) -> Result<Vec<Value>, Value> {
        self.set_read_timeout(timeout)?;
        let mut events = Vec::new();
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        events.push(value);
                    }
                }
                Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => break,
                Ok(Message::Frame(_)) => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == io::ErrorKind::WouldBlock
                        || err.kind() == io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("read CDP event failed: {err}"),
                        &["retry after refreshing browser state"],
                        true,
                    ))
                }
            }
        }
        Ok(events)
    }

    fn read_message(&mut self) -> Result<Value, Value> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str::<Value>(&text).map_err(|err| {
                        browser_error(
                            "CDP_CALL_FAILED",
                            &format!("parse CDP message failed: {err}"),
                            &["retry after refreshing browser state"],
                            true,
                        )
                    });
                }
                Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        "Chrome CDP websocket closed.",
                        &["retry browser_open"],
                        true,
                    ))
                }
                Ok(Message::Frame(_)) => {}
                Err(err) => {
                    return Err(browser_error(
                        "CDP_CALL_FAILED",
                        &format!("read CDP response failed: {err}"),
                        &["retry after refreshing browser state"],
                        true,
                    ))
                }
            }
        }
    }

    fn set_read_timeout(&mut self, timeout: Duration) -> Result<(), Value> {
        match self.socket.get_mut() {
            MaybeTlsStream::Plain(stream) => {
                stream.set_read_timeout(Some(timeout)).map_err(|err| {
                    browser_error(
                        "CDP_CALL_FAILED",
                        &format!("set CDP timeout failed: {err}"),
                        &["retry browser_open"],
                        true,
                    )
                })
            }
            _ => Ok(()),
        }
    }
}
