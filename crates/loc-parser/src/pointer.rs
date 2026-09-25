use anyhow::{anyhow, Result};
use serde_json::Value;

/// 将路径片段编码为 RFC-6901 JSON Pointer token (转义 '~' 和 '/')
pub fn encode_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// 将 RFC-6901 JSON Pointer token 反转义
pub fn decode_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// 解析 RFC-6901 指针路径为 tokens 列表
pub fn parse_pointer(pointer: &str) -> Result<Vec<String>> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    if !pointer.starts_with('/') {
        return Err(anyhow!("Invalid JSON Pointer: must start with '/': {}", pointer));
    }
    let tokens = pointer[1..]
        .split('/')
        .map(decode_token)
        .collect();
    Ok(tokens)
}

/// 在 serde_json::Value 上根据 pointer 获取不可变引用
pub fn get_at_pointer<'a>(root: &'a Value, pointer: &str) -> Result<&'a Value> {
    let tokens = parse_pointer(pointer)?;
    let mut current = root;
    for token in tokens {
        match current {
            Value::Object(map) => {
                current = map.get(&token).ok_or_else(|| anyhow!("Key not found: {}", token))?;
            }
            Value::Array(list) => {
                let idx: usize = token.parse().map_err(|_| anyhow!("Invalid array index: {}", token))?;
                current = list.get(idx).ok_or_else(|| anyhow!("Index out of bounds: {}", idx))?;
            }
            _ => return Err(anyhow!("Cannot traverse into primitive value at: {}", token)),
        }
    }
    Ok(current)
}

/// 在 serde_json::Value 上根据 pointer 精准就地修改叶子节点
pub fn set_at_pointer(root: &mut Value, pointer: &str, new_value: Value) -> Result<()> {
    let tokens = parse_pointer(pointer)?;
    if tokens.is_empty() {
        *root = new_value;
        return Ok(());
    }

    let mut current = root;
    let len = tokens.len();
    for token in &tokens[..len - 1] {
        match current {
            Value::Object(map) => {
                current = map.get_mut(token).ok_or_else(|| anyhow!("Key not found: {}", token))?;
            }
            Value::Array(list) => {
                let idx: usize = token.parse().map_err(|_| anyhow!("Invalid array index: {}", token))?;
                current = list.get_mut(idx).ok_or_else(|| anyhow!("Index out of bounds: {}", idx))?;
            }
            _ => return Err(anyhow!("Cannot traverse into primitive value at: {}", token)),
        }
    }

    let last_token = &tokens[len - 1];
    match current {
        Value::Object(map) => {
            if let Some(field) = map.get_mut(last_token) {
                *field = new_value;
                Ok(())
            } else {
                Err(anyhow!("Target field not found: {}", last_token))
            }
        }
        Value::Array(list) => {
            let idx: usize = last_token.parse().map_err(|_| anyhow!("Invalid array index: {}", last_token))?;
            if let Some(elem) = list.get_mut(idx) {
                *elem = new_value;
                Ok(())
            } else {
                Err(anyhow!("Index out of bounds: {}", idx))
            }
        }
        _ => Err(anyhow!("Cannot set field on primitive value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pointer_get_and_set() -> Result<()> {
        let mut data = json!({
            "dataList": [
                {
                    "id": 1,
                    "content": "안녕하세요",
                    "texts": [
                        { "index": 0, "text": "첫번째" },
                        { "index": 1, "text": "두번째" }
                    ]
                }
            ]
        });

        // 读测试
        let val = get_at_pointer(&data, "/dataList/0/content")?;
        assert_eq!(val.as_str(), Some("안녕하세요"));

        let val_sub = get_at_pointer(&data, "/dataList/0/texts/1/text")?;
        assert_eq!(val_sub.as_str(), Some("두번째"));

        // 改测试
        set_at_pointer(&mut data, "/dataList/0/texts/1/text", json!("第二个"))?;
        let val_after = get_at_pointer(&data, "/dataList/0/texts/1/text")?;
        assert_eq!(val_after.as_str(), Some("第二个"));

        Ok(())
    }
}
