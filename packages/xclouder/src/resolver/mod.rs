
use std::collections::HashMap;

use crate::config::{BucketSource, CloudMagic};

pub fn resolve(
    branch_cloud_source: &HashMap<String, BucketSource>,
    key: &str,
    magics: &[&CloudMagic]
) -> String {
    if !key.chars().all(|c| c.is_ascii()) {
        return key.to_string();
    }

    if key.starts_with("http") || key.starts_with("wxfile") {
        return key.to_string();
    }

    let key = if key.starts_with('/') { &key[1..] } else { key };

    let (key_part, query_part) = key.split_once('?').unwrap_or((key, ""));

    let (cloud_name, _) = if key.starts_with('_') {
        let (cloud_name, key_rest) = key_part.split_once('/').unwrap_or((key_part, ""));
        (cloud_name, key_rest)
    } else {
        // 处理旧格式的 key
        let mut cloud_name = "_cos";
        if key_part.contains("cos") {
            cloud_name = "_cos";
        } else if key_part.contains("tos") {
            cloud_name = "_tos";
        } else if key_part.contains("oss") {
            cloud_name = "_oss";
        }
        (cloud_name, key_part)
    };

    let mut queries = Vec::new();

    if !query_part.is_empty() {
        queries.push(query_part.to_string());
    }

    // 处理魔法参数
    if let Some(bucket_source) = branch_cloud_source.get(cloud_name) {
      let cloud = bucket_source.cloud.as_ref();
      if let Some(cloud) = cloud {
        for magic in magics {
          if let Some(q) = magic.get_magic(cloud) {
            queries.push(q.to_string());
          }
        }
      }
    }

    let base_url = format!(
        "https://{}",
        &branch_cloud_source[cloud_name].cdn_domain.as_ref().unwrap_or(&"".to_string())
    );

    if queries.is_empty() {
        format!("{}/{}", base_url, key_part)
    } else {
        format!("{}/{}?{}", base_url, key_part, queries.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        use std::collections::HashMap;
        use crate::config::{BucketSource, CloudMagic};

        let mut bucket_cloud_source = HashMap::new();
        let mut bucket_source = BucketSource {
            name: "_cos".to_string(),
            cloud: Some("cos".to_string()),
            cdn_domain: Some("example.cos.com".to_string()),
            domain: None,
            fallback: None,
            cloud_name: None,
            grayscale: None,
        };
        bucket_cloud_source.insert("_cos".to_string(), bucket_source);

        let mut cloud_cfg = HashMap::new();
        cloud_cfg.insert("cos".to_string(), "imageMogr2/thumbnail/200".to_string());
        let magic = CloudMagic {
            name: "thumbnail".to_string(),
            cloud_cfg,
        };

        let key = "_cos/test.jpg";

        let url = resolve(&bucket_cloud_source, key, &[&magic]);
        assert_eq!(
            url,
            "https://example.cos.com/_cos/test.jpg?imageMogr2/thumbnail/200"
        );
    }
}
