use super::*;
use getrandom::getrandom;
use serde_json::Value;
use url::Url;

const HYPERWG_MARKER: &str = "hyperwg";
const HYPERWG_VERSION: &str = "2";
const HYPERWG_CLIENT_OBFUSCATION: &str = "hyperwg-client-v2";
const HYPERWG_DEVICE_ID_FILE: &str = "hyperwg_device_id.txt";
const HYPERWG_RESPONSE_LIMIT: u64 = 1024 * 1024;
const HYPERWG_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const HYPERWG_TRAFFIC_PROFILES: [&str; 5] = ["off", "interactive", "web", "streaming", "mixed"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct HyperwgProvisioningConfig {
    enrollment_url: Url,
    access_key: String,
}

#[derive(Debug)]
pub(super) struct PreparedHyperwgConfig {
    pub native_config: String,
    pub evicted_device_id: Option<String>,
}

pub(super) fn is_hyperwg_config_path(path: &str) -> bool {
    fs::read_to_string(path)
        .map(|content| is_hyperwg_import_content(&content))
        .unwrap_or(false)
}

fn is_hyperwg_import_content(content: &str) -> bool {
    is_hyperwg_config_content(content) || is_hyperwg_native_config_content(content)
}

pub(super) fn is_hyperwg_config_content(content: &str) -> bool {
    first_meaningful_line(content).is_some_and(|line| line.eq_ignore_ascii_case(HYPERWG_MARKER))
}

pub(super) fn is_hyperwg_native_config_content(content: &str) -> bool {
    let mut current_section = "";
    for raw_line in content.lines() {
        let line = raw_line.split(['#', ';']).next().unwrap_or_default().trim();
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line.trim_matches(['[', ']']).trim();
            continue;
        }
        if current_section.eq_ignore_ascii_case("Interface")
            && line
                .split_once('=')
                .is_some_and(|(key, _value)| key.trim().eq_ignore_ascii_case("TrafficMorpher"))
        {
            return true;
        }
    }
    false
}

pub(super) fn prepare_hyperwg_runtime_config(
    provisioning_content: &str,
) -> Result<PreparedHyperwgConfig, String> {
    let provisioning = parse_hyperwg_config(provisioning_content)?;
    let device_id = load_or_create_hyperwg_device_id()?;
    enroll_hyperwg(&provisioning, &device_id)
}

fn first_meaningful_line(content: &str) -> Option<&str> {
    content
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').trim())
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
}

fn parse_hyperwg_config(content: &str) -> Result<HyperwgProvisioningConfig, String> {
    if !is_hyperwg_config_content(content) {
        return Err("В конфигурации отсутствует маркер HyperWG".to_string());
    }

    let mut marker_seen = false;
    let mut version = None;
    let mut endpoint = None;
    let mut access_key = None;

    for raw_line in content.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if !marker_seen {
            marker_seen = true;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            return Err("Секции не поддерживаются в конфигурации HyperWG".to_string());
        }

        let (raw_key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("Некорректная строка HyperWG: {}", line))?;
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(format!("Пустое поле HyperWG: {}", key));
        }

        let target = match key.as_str() {
            "version" => &mut version,
            "endpoint" => &mut endpoint,
            "accesskey" => &mut access_key,
            _ => return Err(format!("Неподдерживаемое поле HyperWG: {}", raw_key.trim())),
        };
        if target.replace(value.to_string()).is_some() {
            return Err(format!("Поле HyperWG указано повторно: {}", raw_key.trim()));
        }
    }

    if version.as_deref() != Some(HYPERWG_VERSION) {
        return Err("Поддерживается только конфигурация HyperWG Version = 2".to_string());
    }
    let endpoint = endpoint.ok_or_else(|| "В HyperWG не указан Endpoint".to_string())?;
    let access_key = access_key.ok_or_else(|| "В HyperWG не указан AccessKey".to_string())?;
    let mut enrollment_url = Url::parse(&endpoint)
        .map_err(|_| "Endpoint HyperWG должен быть абсолютным HTTP(S) URL".to_string())?;
    if !matches!(enrollment_url.scheme(), "http" | "https")
        || enrollment_url.host_str().is_none()
        || !enrollment_url.username().is_empty()
        || enrollment_url.password().is_some()
        || enrollment_url.query().is_some()
        || enrollment_url.fragment().is_some()
    {
        return Err(
            "Endpoint HyperWG должен быть абсолютным HTTP(S) URL без credentials, query и fragment"
                .to_string(),
        );
    }

    let path = enrollment_url.path().trim_end_matches('/');
    let lower_path = path.to_ascii_lowercase();
    let enrollment_path = if lower_path.ends_with("/api/enroll") {
        path.to_string()
    } else if lower_path.ends_with("/api") {
        format!("{}/enroll", path)
    } else if path.is_empty() {
        "/api/enroll".to_string()
    } else {
        format!("{}/api/enroll", path)
    };
    enrollment_url.set_path(&enrollment_path);

    Ok(HyperwgProvisioningConfig {
        enrollment_url,
        access_key,
    })
}

fn load_or_create_hyperwg_device_id() -> Result<String, String> {
    let path = managed_configs_dir().join(HYPERWG_DEVICE_ID_FILE);
    if let Ok(existing) = fs::read_to_string(&path) {
        let existing = existing.trim();
        if is_valid_device_id(existing) {
            return Ok(existing.to_string());
        }
    }

    let mut bytes = [0u8; 16];
    getrandom(&mut bytes).map_err(|error| {
        format!(
            "Не удалось создать идентификатор устройства HyperWG: {}",
            error
        )
    })?;
    let mut device_id = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut device_id, "{:02x}", byte);
    }
    fs::write(&path, &device_id).map_err(|error| {
        format!(
            "Не удалось сохранить идентификатор устройства HyperWG: {}",
            error
        )
    })?;
    Ok(device_id)
}

fn is_valid_device_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn enroll_hyperwg(
    provisioning: &HyperwgProvisioningConfig,
    device_id: &str,
) -> Result<PreparedHyperwgConfig, String> {
    let name_prefix = device_id.get(..8).unwrap_or(device_id);
    let request_body = serde_json::json!({
        "access_key": provisioning.access_key,
        "device_id": device_id,
        "name": format!("vpnfybot-windows-{}", name_prefix),
    })
    .to_string();
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HYPERWG_REQUEST_TIMEOUT)
        .timeout_read(HYPERWG_REQUEST_TIMEOUT)
        .timeout_write(HYPERWG_REQUEST_TIMEOUT)
        .redirects(0)
        .build();
    let response = match agent
        .post(provisioning.enrollment_url.as_str())
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .send_string(&request_body)
    {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body = read_limited_response(response)?;
            let decoded = serde_json::from_str::<Value>(&body).ok();
            let message = decoded
                .as_ref()
                .and_then(|json| json.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("HyperWG enrollment отклонён сервером");
            let code = decoded
                .as_ref()
                .and_then(|json| json.get("code"))
                .and_then(Value::as_str);
            return Err(match code {
                Some(code) => format!("{} (HTTP {}, {})", message, status, code),
                None => format!("{} (HTTP {})", message, status),
            });
        }
        Err(ureq::Error::Transport(error)) => {
            return Err(format!("Не удалось подключиться к HyperWG API: {}", error));
        }
    };

    let body = read_limited_response(response)?;
    let decoded: Value = serde_json::from_str(&body)
        .map_err(|_| "HyperWG API вернул некорректный JSON".to_string())?;
    let tunnel = decoded
        .get("tunnel")
        .and_then(Value::as_object)
        .ok_or_else(|| "HyperWG API не вернул параметры туннеля".to_string())?;
    if tunnel.get("protocol").and_then(Value::as_str) != Some(HYPERWG_MARKER) {
        return Err("HyperWG API вернул конфигурацию другого протокола".to_string());
    }
    if tunnel.get("protocol_version").and_then(Value::as_str) != Some(HYPERWG_VERSION) {
        return Err("HyperWG API вернул неподдерживаемую версию протокола".to_string());
    }
    if tunnel.get("client_obfuscation").and_then(Value::as_str) != Some(HYPERWG_CLIENT_OBFUSCATION)
    {
        return Err("HyperWG API вернул неподдерживаемую схему клиентской обфускации".to_string());
    }
    let traffic_morpher = tunnel
        .get("traffic_morpher")
        .and_then(Value::as_str)
        .filter(|profile| HYPERWG_TRAFFIC_PROFILES.contains(profile))
        .ok_or_else(|| "HyperWG API вернул неподдерживаемый профиль TrafficMorpher".to_string())?;
    let native_config = decoded
        .get("native_config")
        .and_then(Value::as_str)
        .filter(|config| !config.is_empty())
        .ok_or_else(|| "HyperWG API не вернул native_config".to_string())?;
    validate_native_config(native_config)?;
    let native_config = apply_hyperwg_client_profile(native_config, traffic_morpher)?;
    validate_native_config(&native_config)?;
    let evicted_device_id = decoded
        .get("evicted_device")
        .and_then(Value::as_object)
        .and_then(|device| device.get("external_id"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(PreparedHyperwgConfig {
        native_config,
        evicted_device_id,
    })
}

fn read_limited_response(response: ureq::Response) -> Result<String, String> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(HYPERWG_RESPONSE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Не удалось прочитать ответ HyperWG API: {}", error))?;
    if bytes.len() as u64 > HYPERWG_RESPONSE_LIMIT {
        return Err("Ответ HyperWG API превышает 1 MiB".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "HyperWG API вернул ответ не в UTF-8".to_string())
}

fn validate_native_config(config: &str) -> Result<(), String> {
    let mut current_section = "";
    let mut interface_count = 0usize;
    let mut peer_count = 0usize;
    let mut private_key = false;
    let mut public_key = false;
    let mut endpoint = false;

    for raw_line in config.lines() {
        let line = raw_line.split(['#', ';']).next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line.trim_matches(['[', ']']).trim();
            if current_section.eq_ignore_ascii_case("Interface") {
                interface_count += 1;
            } else if current_section.eq_ignore_ascii_case("Peer") {
                peer_count += 1;
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        if current_section.eq_ignore_ascii_case("Interface")
            && key.trim().eq_ignore_ascii_case("PrivateKey")
        {
            private_key = true;
        } else if current_section.eq_ignore_ascii_case("Peer")
            && key.trim().eq_ignore_ascii_case("PublicKey")
        {
            public_key = true;
        } else if current_section.eq_ignore_ascii_case("Peer")
            && key.trim().eq_ignore_ascii_case("Endpoint")
        {
            endpoint = true;
        }
    }

    if interface_count != 1 || peer_count == 0 || !private_key || !public_key || !endpoint {
        return Err("HyperWG API вернул неполную native-конфигурацию туннеля".to_string());
    }
    Ok(())
}

fn secure_random_inclusive(minimum: u32, maximum: u32) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    getrandom(&mut bytes)
        .map_err(|error| format!("Не удалось получить случайность для HyperWG: {}", error))?;
    let span = maximum - minimum + 1;
    Ok(minimum + (u32::from_le_bytes(bytes) % span))
}

fn apply_hyperwg_client_profile(config: &str, traffic_morpher: &str) -> Result<String, String> {
    let junk_minimum = secure_random_inclusive(8, 24)?;
    let junk_maximum = secure_random_inclusive(64, 160)?;
    let junk_count = secure_random_inclusive(4, 12)?;
    let signature_length = secure_random_inclusive(48, 160)?;
    let keepalive = secure_random_inclusive(20, 30)?;
    apply_hyperwg_client_profile_with_values(
        config,
        traffic_morpher,
        junk_count,
        junk_minimum,
        junk_maximum,
        signature_length,
        keepalive,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_hyperwg_client_profile_with_values(
    config: &str,
    traffic_morpher: &str,
    junk_count: u32,
    junk_minimum: u32,
    junk_maximum: u32,
    signature_length: u32,
    keepalive: u32,
) -> Result<String, String> {
    if !HYPERWG_TRAFFIC_PROFILES.contains(&traffic_morpher) {
        return Err("Неподдерживаемый профиль TrafficMorpher".to_string());
    }
    let line_ending = if config.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut output = Vec::new();
    let mut current_section = "";
    let mut interface_seen = false;
    let mut peer_seen = false;
    let mut keepalive_written = false;

    for raw_line in config.lines() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed.trim_matches(['[', ']']).trim();
            output.push(raw_line.to_string());
            if current_section.eq_ignore_ascii_case("Interface") {
                if interface_seen {
                    return Err(
                        "HyperWG native_config содержит несколько секций Interface".to_string()
                    );
                }
                interface_seen = true;
                output.push(format!("TrafficMorpher = {}", traffic_morpher));
                output.push(format!("Jc = {}", junk_count));
                output.push(format!("Jmin = {}", junk_minimum));
                output.push(format!("Jmax = {}", junk_maximum));
                output.push(format!("I1 = <r {}>", signature_length));
            } else if current_section.eq_ignore_ascii_case("Peer") {
                peer_seen = true;
            }
            continue;
        }

        let key = trimmed
            .split_once('=')
            .map(|(key, _value)| key.trim().to_ascii_lowercase());
        if current_section.eq_ignore_ascii_case("Interface")
            && key.as_deref().is_some_and(|key| {
                matches!(
                    key,
                    "i1" | "i2" | "i3" | "i4" | "i5" | "jc" | "jmin" | "jmax" | "trafficmorpher"
                )
            })
        {
            continue;
        }
        if current_section.eq_ignore_ascii_case("Peer")
            && key.as_deref() == Some("persistentkeepalive")
        {
            if !keepalive_written {
                output.push(format!("PersistentKeepalive = {}", keepalive));
                keepalive_written = true;
            }
            continue;
        }
        output.push(raw_line.to_string());
    }

    if !interface_seen || !peer_seen {
        return Err("В native_config HyperWG отсутствует Interface или Peer".to_string());
    }
    if !keepalive_written {
        let peer_index = output
            .iter()
            .position(|line| line.trim().eq_ignore_ascii_case("[Peer]"))
            .ok_or_else(|| "В native_config HyperWG отсутствует Peer".to_string())?;
        output.insert(
            peer_index + 1,
            format!("PersistentKeepalive = {}", keepalive),
        );
    }
    Ok(output.join(line_ending) + line_ending)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const NATIVE_CONFIG: &str = "[Interface]\nAddress = 10.8.0.2/32\nPrivateKey = private\nJc = 99\nI2 = <r 12>\nHeaderProtectionKey = abc\n\n[Peer]\nPublicKey = public\nEndpoint = 127.0.0.1:51820\nPersistentKeepalive = 25\n";

    #[test]
    fn parses_v2_and_builds_enrollment_endpoint() {
        let parsed = parse_hyperwg_config(
            "# comment\nHyperWG\nVersion = 2\nEndpoint = https://vpn.example.test/control\nAccessKey = secret\n",
        )
        .unwrap();
        assert_eq!(
            parsed.enrollment_url.as_str(),
            "https://vpn.example.test/control/api/enroll"
        );
        assert_eq!(parsed.access_key, "secret");
    }

    #[test]
    fn preserves_existing_enrollment_endpoint() {
        let parsed = parse_hyperwg_config(
            "hyperwg\nVersion=2\nEndpoint=https://vpn.example.test/api/enroll\nAccessKey=secret\n",
        )
        .unwrap();
        assert_eq!(
            parsed.enrollment_url.as_str(),
            "https://vpn.example.test/api/enroll"
        );
    }

    #[test]
    fn accepts_utf8_bom_in_exported_provisioning_config() {
        let parsed = parse_hyperwg_config(
            "\u{feff}# exported by the panel\nhyperwg\nVersion = 2\nEndpoint = https://vpn.example.test\nAccessKey = secret\n",
        )
        .unwrap();
        assert_eq!(
            parsed.enrollment_url.as_str(),
            "https://vpn.example.test/api/enroll"
        );
    }

    #[test]
    fn recognizes_native_hyperwg_without_reclassifying_legacy_awg() {
        let native_hyperwg =
            NATIVE_CONFIG.replacen("[Interface]\n", "[Interface]\nTrafficMorpher = mixed\n", 1);
        assert!(is_hyperwg_import_content(&native_hyperwg));
        assert!(is_hyperwg_native_config_content(&native_hyperwg));
        assert!(!is_hyperwg_import_content(NATIVE_CONFIG));
    }

    #[test]
    fn rejects_legacy_or_extended_provisioning_configs() {
        for config in [
            "hyperwg\nVersion=1\nEndpoint=https://vpn.example.test\nAccessKey=x\n",
            "hyperwg\nVersion=2\nEndpoint=https://vpn.example.test\nAccessKey=x\nJc=5\n",
            "hyperwg\nVersion=2\nEndpoint=https://user@vpn.example.test\nAccessKey=x\n",
        ] {
            assert!(parse_hyperwg_config(config).is_err());
        }
    }

    #[test]
    fn replaces_client_owned_values_and_keeps_server_parameters() {
        let result =
            apply_hyperwg_client_profile_with_values(NATIVE_CONFIG, "mixed", 7, 10, 120, 80, 29)
                .unwrap();
        assert!(result.contains("TrafficMorpher = mixed"));
        assert!(result.contains("Jc = 7\nJmin = 10\nJmax = 120\nI1 = <r 80>"));
        assert!(!result.contains("Jc = 99"));
        assert!(!result.contains("I2 ="));
        assert!(result.contains("HeaderProtectionKey = abc"));
        assert!(result.contains("PersistentKeepalive = 29"));
    }

    #[test]
    fn validates_minimum_native_tunnel_shape() {
        assert!(validate_native_config(NATIVE_CONFIG).is_ok());
        assert!(validate_native_config("[Interface]\nPrivateKey=x\n").is_err());
    }

    #[test]
    fn enrolls_and_applies_the_advertised_client_profile() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 2048];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .unwrap()
                + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap();
            while request.len() - header_end < content_length {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
            }
            let payload: Value =
                serde_json::from_slice(&request[header_end..header_end + content_length]).unwrap();
            assert_eq!(
                payload.get("access_key").and_then(Value::as_str),
                Some("test-access")
            );
            assert_eq!(
                payload.get("device_id").and_then(Value::as_str),
                Some("0123456789abcdef0123456789abcdef")
            );

            let body = serde_json::json!({
                "tunnel": {
                    "protocol": "hyperwg",
                    "protocol_version": "2",
                    "traffic_morpher": "mixed",
                    "client_obfuscation": "hyperwg-client-v2"
                },
                "native_config": NATIVE_CONFIG,
                "evicted_device": {"external_id": "previous-device"}
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let provisioning = parse_hyperwg_config(&format!(
            "hyperwg\nVersion = 2\nEndpoint = http://{}\nAccessKey = test-access\n",
            address
        ))
        .unwrap();
        let enrolled = enroll_hyperwg(&provisioning, "0123456789abcdef0123456789abcdef").unwrap();
        server.join().unwrap();

        assert!(is_hyperwg_native_config_content(&enrolled.native_config));
        assert!(enrolled.native_config.contains("TrafficMorpher = mixed"));
        assert!(!enrolled.native_config.contains("Jc = 99"));
        assert_eq!(
            enrolled.evicted_device_id.as_deref(),
            Some("previous-device")
        );
    }
}
