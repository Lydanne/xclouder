mod cloud_client;
mod error;
mod strategy;
pub mod resolver;
mod events;
mod network;
use config::{BucketSource, CloudMagic};
pub use network::NetworkInfo;
mod utils;
mod config;
mod inner;

use cloud_client::{CloudClient, UploadOpts};
use error::XResult;
use strategy::{Strategy, UrlRes};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

pub struct Clouder<'a> {
    client: CloudClient<'a>,
}

impl<'a> Clouder<'a> {
    pub fn new(opts: ClouderOptions) -> Self {
        let mut client = CloudClient::new(opts.native);
        
        for strategy in opts.strategy {
            client.load_strategy(strategy);
        }
        
        Self { client }
    }

    pub fn xcm(&self) -> &HashMap<String, CloudMagic> {
        &self.client.cloud_magics_map
    }

    pub fn xc(&self) -> &HashMap<String, HashMap<String, BucketSource>> {
        &self.client.branch_cloud_source
    }

    pub fn init(&mut self, remote: Option<String>, config: serde_json::Value) -> &mut Self {
        self.client.init(remote, config);
        self
    }

    pub async fn upload(
        &'a self,
        bucket: &str,
        file_path: &str,
        filename: String,
        opts: UploadOptions,
    ) -> XResult<String> {
        println!("[XClouder] upload {} {}", bucket, file_path);
        let default_cloud = "_main".to_string();
        let cloud_name = opts.cloud_name.as_ref().unwrap_or(&default_cloud);
        let bucket_source = self.client.current_bucket_source(bucket, cloud_name, true)?;
        let up_id = chrono::Utc::now().timestamp_millis();
        
        let key = format!("{}/{}", cloud_name, filename);
        
        self.client.upload_fn(UploadOpts {
            bucket_source,
            bucket: bucket.to_string(),
            filename,
            file_path: file_path.to_string(),
            key: key,
            on_progress: opts.on_progress,
            up_id,
            disable_retry: opts.disable_retry,
            manual_retry: opts.manual_retry,
        }).await
    }

    pub fn resolve(&self, bucket: &str, key: &str, magics: &[&str]) -> XResult<String> {
        let branch_cloud_source = self.client.current_branch_cloud_source(bucket)?;
        let magics = self.client.current_magics(magics)?;
        Ok(crate::resolver::resolve(branch_cloud_source, key, &magics))
    }

    pub fn is_xclouder(&self, key: &str) -> bool {
        self.client.is_xclouder(key)
    }

    pub fn take_cloud(&self, key: &str) -> Option<String> {
        self.client.take_cloud(key)
    }

    pub fn simple_key(&self, key: &str) -> String {
        self.client.simple_key(key)
    }
}

pub struct ClouderOptions {
    pub strategy: Vec<Box<dyn Strategy>>,
    pub native: Box<dyn Native>,
}

pub struct UploadOptions {
    pub cloud_name: Option<String>,
    pub on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
    pub disable_retry: bool,
    pub manual_retry: bool,
    pub openid: Option<String>,
}

#[derive(Clone)]
pub struct UploadArgs {
    pub url: String,
    pub name: String,
    pub file_path: String,
    pub form_data: Value,
    pub on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
}

impl std::fmt::Debug for UploadArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadArgs")
            .field("url", &self.url)
            .field("name", &self.name)
            .field("file_path", &self.file_path)
            .field("form_data", &self.form_data)
            .field("on_progress", &self.on_progress.as_ref().map(|_| "Fn(f32)"))
            .finish()
    }
}

#[derive(Clone)]
pub struct RequestArgs {
    pub method: String,
    pub url: String,
    pub enable_cache: bool,
    pub timeout: u32,
    pub response_type: String,
}

impl std::fmt::Debug for RequestArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestArgs")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("enable_cache", &self.enable_cache)
            .field("timeout", &self.timeout)
            .field("response_type", &self.response_type)
            .finish()
    }
}

#[async_trait::async_trait]
pub trait Native: Send + Sync {
    async fn upload_file(&self, args: UploadArgs) -> XResult<()>;
    async fn request(&self, args: RequestArgs) -> XResult<serde_json::Value>;
    fn set_storage(&self, key: &str, value: serde_json::Value);
    fn get_storage(&self, key: &str) -> Option<serde_json::Value>;
    fn del_storage(&self, key: &str);
    fn resolve_fallback(&self, bucket: &str, key: &str) -> String;
    async fn check_network(&self) -> XResult<NetworkInfo>;
    async fn check_dns(&self, domain: &str) -> XResult<bool>;
}

pub use config::Config;

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use config::BucketSource;

    use super::*;
    use std::sync::Arc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockNative {
        storage: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    }

    impl MockNative {
        fn new() -> Self {
            Self {
                storage: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl Native for MockNative {
        async fn upload_file(&self, args: UploadArgs) -> XResult<()> {
            println!("[Mock] upload_file: {:?}", args);
            Ok(())
        }

        async fn request(&self, args: RequestArgs) -> XResult<serde_json::Value> {
            println!("[Mock] request: {:?}", args);
            Ok(serde_json::json!({
                "expireAt": chrono::Utc::now().timestamp() + 3600,
                "mergeFormData": {
                    "token": "mock_token"
                }
            }))
        }

        fn set_storage(&self, key: &str, value: serde_json::Value) {
            self.storage.lock().unwrap().insert(key.to_string(), value);
        }

        fn get_storage(&self, key: &str) -> Option<serde_json::Value> {
            self.storage.lock().unwrap().get(key).cloned()
        }

        fn del_storage(&self, key: &str) {
            self.storage.lock().unwrap().remove(key);
        }

        fn resolve_fallback(&self, bucket: &str, key: &str) -> String {
            format!("https://mock.{}.com/{}", bucket, key)
        }

        async fn check_network(&self) -> XResult<NetworkInfo> {
            Ok(NetworkInfo {
                has_system_proxy: false,
                signal_strength: 100,
                network_type: "wifi".to_string(),
                dns_error: false,
                check_error: None,
            })
        }

        async fn check_dns(&self, domain: &str) -> XResult<bool> {
            Ok(true)
        }
    }

    // 添加个 Mock 策略实现
    struct MockStrategy {
        name: String,
        native: Option<Box<dyn Native>>,
    }

    impl MockStrategy {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                native: None,
            }
        }
    }

    #[async_trait]
    impl Strategy for MockStrategy {
        fn name(&self) -> &str {
            &self.name
        }

        fn load_native(&mut self, native: Box<dyn Native>) {
            self.native = Some(native);
        }

        fn storage_key(&self, bucket: &Value) -> String {
            format!("sts:mock:{}", bucket["name"].as_str().unwrap_or(""))
        }

        fn domain_parser(&self, domain: &str) -> Value {
            serde_json::json!({
                "mock": true,
                "domain": domain
            })
        }

        async fn get_sts(&self, bucket_source: &BucketSource, opts: &UploadOpts<'_>) -> XResult<Value> {
            Ok(serde_json::json!({
                "mergeFormData": {
                    "token": "mock_sts_token"
                }
            }))
        }

        async fn upload(&self, bucket_source: &BucketSource, sts: Value, opts: &UploadOpts<'_>) -> XResult<UrlRes> {
            Ok(UrlRes {
                base_url: format!("https://{}", bucket_source.domain.clone().unwrap()),
                key: opts.key.clone(),
                domain: bucket_source.domain.clone().unwrap(),
                bucket: bucket_source.name.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_upload() {
        let native = Box::new(MockNative::new());
        let opts = ClouderOptions {
            strategy: vec![Box::new(MockStrategy::new("mock"))],
            native,
        };
        
        let mut clouder = Clouder::new(opts);
        clouder.init(None, serde_json::json!({
            "cloudSource": [{
                "name": "_mock",
                "cloud": "mock",
                "buckets": [{
                    "name": "test",
                    "domain": "test.mock.com",
                    "cdnDomain": "test.mock.com",
                    "cloudName": "_mock",
                    "cloud": "mock"
                }]
            }],
            "cloudMagics": []
        }));

        let result = clouder.upload(
            "test",
            "test.jpg",
            "test.jpg".to_string(),
            UploadOptions {
                cloud_name: Some("_mock".to_string()),
                on_progress: None,
                disable_retry: false,
                manual_retry: false,
                openid: Some("test_user".to_string()),
            }
        ).await;

        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("test.mock.com"));
    }

    #[test]
    fn test_resolve() {
        let native = Box::new(MockNative::new());
        let opts = ClouderOptions {
            strategy: vec![Box::new(MockStrategy::new("mock"))],
            native,
        };
        
        let mut clouder = Clouder::new(opts);
        clouder.init(None, serde_json::json!({
            "cloudSource": [{
                "name": "_mock",
                "cloud": "mock",
                "buckets": [{
                    "name": "test",
                    "domain": "test.mock.com",
                    "cdnDomain": "cdn.mock.com",
                    "cloudName": "_mock",
                    "cloud": "mock"
                }]
            }],
            "cloudMagics": [{
                "name": "thumbnail",
                "cloudCfg": {
                    "mock": "size=100x100"
                }
            }]
        }));

        let result = clouder.resolve(
            "test",
            "_mock/test.jpg",
            &["thumbnail"]
        );

        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("cdn.mock.com"));
        assert!(url.contains("size=100x100"));
    }

    #[tokio::test]
    async fn test_upload_with_auto_name() {
        let native = Box::new(MockNative::new());
        let opts = ClouderOptions {
            strategy: vec![Box::new(MockStrategy::new("mock"))],
            native,
        };
        
        let mut clouder = Clouder::new(opts);
        clouder.init(None, serde_json::json!({
            "cloudSource": [{
                "name": "_mock",
                "cloud": "mock",
                "buckets": [{
                    "name": "test",
                    "domain": "test.mock.com",
                    "cdnDomain": "test.mock.com",
                    "cloudName": "_mock",
                    "cloud": "mock"
                }]
            }],
            "cloudMagics": []
        }));

        let result = clouder.upload(
            "test",
            "test.jpg",
            utils::fill_name("test.jpg", "test_user"),
            UploadOptions {
                cloud_name: Some("_mock".to_string()),
                on_progress: None,
                disable_retry: false,
                manual_retry: false,
                openid: Some("test_user".to_string()),
            }
        ).await;

        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("test.mock.com"));
        assert!(url.contains("test_user/"));
    }

    #[tokio::test]
    async fn test_upload_with_retry() {
        let native = Box::new(MockNative::new());
        let opts = ClouderOptions {
            strategy: vec![Box::new(MockStrategy::new("mock"))],
            native,
        };
        
        let mut clouder = Clouder::new(opts);
        clouder.init(None, serde_json::json!({
            "cloudSource": [{
                "name": "_mock",
                "cloud": "mock",
                "buckets": [{
                    "name": "test",
                    "domain": "test.mock.com",
                    "cdnDomain": "test.mock.com",
                    "cloudName": "_mock",
                    "cloud": "mock",
                    "fallback": "_mock2.test"
                }]
            }, {
                "name": "_mock2",
                "cloud": "mock",
                "buckets": [{
                    "name": "test",
                    "domain": "test2.mock.com",
                    "cdnDomain": "test2.mock.com",
                    "cloudName": "_mock2",
                    "cloud": "mock"
                }]
            }],
            "cloudMagics": []
        }));

        let result = clouder.upload(
            "test",
            "test.jpg",
            "test.jpg".to_string(),
            UploadOptions {
                cloud_name: Some("_mock".to_string()),
                on_progress: None,
                disable_retry: false,
                manual_retry: false,
                openid: Some("test_user".to_string()),
            }
        ).await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_key_operations() {
        let native = Box::new(MockNative::new());
        let opts = ClouderOptions {
            strategy: vec![Box::new(MockStrategy::new("mock"))],
            native,
        };
        
        let clouder = Clouder::new(opts);

        assert!(clouder.is_xclouder("_mock/test.jpg"));
        assert!(!clouder.is_xclouder("test.jpg"));

        assert_eq!(clouder.take_cloud("_mock/test.jpg").unwrap(), "_mock");
        assert_eq!(clouder.take_cloud("/_mock/test.jpg").unwrap(), "_mock");
        assert!(clouder.take_cloud("test.jpg").is_none());

        assert_eq!(clouder.simple_key("_mock/test.jpg"), "test.jpg");
        assert_eq!(clouder.simple_key("/_mock/test.jpg"), "test.jpg");
        assert_eq!(clouder.simple_key("test.jpg"), "test.jpg");
    }
}
