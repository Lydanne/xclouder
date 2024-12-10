use std::collections::HashMap;
use crate::{config::{BucketSource, CloudMagic, CloudSource}, error::{XError, XResult}, strategy::{Strategy, UrlRes}, Config, Native};
use serde_json::Value;
use crate::events::Emitter;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use std::sync::Mutex;

pub struct CloudClient<'a> {
    pub native: Box<dyn Native>,
    pub config: Option<Config>,
    pub local_config: Option<Config>,
    
    pub cloud_source_map: HashMap<String, CloudSource>,
    pub cloud_magics_map: HashMap<String, CloudMagic>,
    pub branch_cloud_source: HashMap<String, HashMap<String, BucketSource>>,
    pub cloud_strategy_map: HashMap<String, Box<dyn Strategy>>,
    pub manual_retry_map: Arc<Mutex<HashMap<String, UploadOpts<'a>>>>,
    pub em_upload_end: Emitter, 
    pub em_upload_begin: Emitter,
    pub em_loaded_remote_config: Emitter,
}

impl<'a> CloudClient<'a> {
    pub fn new(native: Box<dyn Native>) -> Self {
        Self {
            native,
            config: None,
            local_config: None,
            cloud_source_map: HashMap::new(),
            cloud_magics_map: HashMap::new(),
            branch_cloud_source: HashMap::new(),
            cloud_strategy_map: HashMap::new(),
            manual_retry_map: Arc::new(Mutex::new(HashMap::new())),
            em_upload_end: Emitter::new(),
            em_upload_begin: Emitter::new(),
            em_loaded_remote_config: Emitter::new(),
        }
    }

    pub fn load_strategy(&mut self, strategy: Box<dyn Strategy>) {
        let name = strategy.name();
        self.cloud_strategy_map.insert(name.to_string(), strategy);
    }

    pub fn init(&mut self, remote: Option<String>, local_config: Value) {
        // 实现初始化逻辑
        self.load_conf(&local_config, &local_config);
    }

    pub fn current_bucket_source(&self, bucket: &str, cloud_name: &str, auto_feedback: bool) -> XResult<&BucketSource> {
        let config = self.config.as_ref().ok_or_else(|| XError::InvalidConfig)?;
        
        let cloud_source = config.get_cloud_source(cloud_name)
            .ok_or_else(|| XError::CloudNotFound)?;
        
        let mut bucket_source = cloud_source.buckets.iter()
            .find(|b| b.name == bucket)
            .ok_or_else(|| XError::BucketNotFound(bucket.to_string()))?;
                
        if auto_feedback {
            if bucket_source.domain.is_none() || 
               bucket_source.grayscale.map_or(false, |v| !Self::when_percent(v)) {
                // 切换到 fallback 域名
                if bucket_source.fallback.is_some() {
                    if let Some((fallback_source, fallback_bucket)) = config.resolve_fallback(cloud_source, &bucket_source.name) {
                        bucket_source = fallback_bucket;
                    }
                }
            }
        }
        
        Ok(bucket_source)
    }

    pub fn current_branch_cloud_source(&self, bucket: &str) -> XResult<&HashMap<String, BucketSource>> {
        let branch_cloud_source = self.branch_cloud_source.get(bucket).ok_or_else(|| XError::BucketNotFound(bucket.to_string()))?;
        Ok(branch_cloud_source)
    }
    
    pub async fn upload_fn(&self, mut opts: UploadOpts<'a>) -> XResult<String> {
        self.em_upload_begin.emit("upload_begin", serde_json::json!({
            "opts": &opts
        })).await;

        {
            let retry_map = self.manual_retry_map.lock().map_err(|_| XError::InvalidConfig)?;
            if let Some(manual_retry_opts) = retry_map.get(&opts.file_path) {
                if opts.manual_retry {
                    opts = manual_retry_opts.clone();
                }
            }
        }

        if !opts.file_path.is_ascii() {
            return Err(XError::UploadFailed("Invalid file path".to_string()));
        }

        println!("[XClouder] uploadFn {:?}", opts);
        
        let mut bucket_source = opts.bucket_source;
        let mut errors = Vec::new();
        let mut retry_count = 0;

        loop {
          let cloud = bucket_source.cloud.as_ref().ok_or_else(|| XError::InvalidConfig)?;
            let cloud_strategy = self.get_cloud_strategy(cloud)?;

            match cloud_strategy.get_sts(bucket_source, &opts).await {
                Ok(sts) => {
                    match cloud_strategy.upload(&bucket_source, sts, &opts).await {
                        Ok(url_res) => {
                            self.em_upload_end.emit("upload_end", serde_json::json!({
                                "opts": &opts,
                                "url": url_res.to_string()
                            })).await;
                            return Ok(url_res.to_string());
                        }
                        Err(err) => {
                            errors.push(err.clone());
                            if opts.disable_retry {
                                break;
                            }
                            retry_count += 1;
                            if retry_count > 5 {
                                break;
                            }

                            // 尝试切换域名
                            if retry_count > 3 {
                                if let Ok(new_source) = self.try_switch_domain(&bucket_source).await {
                                    bucket_source = new_source;
                                    continue;
                                }
                            }

                            // 检查网络状态
                            let network = crate::network::check_network(&*self.native).await;
                            if network.network_type == "none" {
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    errors.push(err);
                    break;
                }
            }
        }

        if opts.manual_retry {
            let mut retry_map = self.manual_retry_map.lock().map_err(|_| XError::InvalidConfig)?;
            retry_map.insert(opts.file_path.clone(), opts.clone());
        }

        let err = XError::UploadFailed(format!("Upload failed after {} retries", retry_count));
        self.em_upload_end.emit("upload_end", serde_json::json!({
            "opts": opts,
            "error": err.to_string()
        })).await;
        
        Err(err)
    }

    pub fn get_cloud_strategy(&self, cloud: &str) -> XResult<&Box<dyn Strategy>> {
        self.cloud_strategy_map.get(cloud)
            .ok_or_else(|| XError::CloudNotFound)
    }

    pub fn current_cloud_source(&self, cloud_name: &str) -> XResult<Value> {
        let name = if cloud_name.starts_with('_') {
            cloud_name
        } else {
            &format!("_{}", cloud_name)
        };
        
        self.cloud_source_map.get(name)
            .cloned()
            .ok_or_else(|| XError::CloudNotFound)
            .map(|source| serde_json::to_value(source).unwrap())
    }

    pub fn load_conf(&mut self, config: &Value, local_config: &Value) {
        let config = Config::from_json(config.clone()).unwrap_or_else(|_| Config {
            cloud_source: vec![],
            cloud_magics: vec![],
        });
        let local_config = Config::from_json(local_config.clone()).unwrap_or_else(|_| Config {
            cloud_source: vec![],
            cloud_magics: vec![],
        });

        self.config = Some(config.clone());
        self.local_config = Some(local_config.clone());

        let mut branch_cloud_source = HashMap::new();
        
        // 加载云源配置
        for source in &config.cloud_source {
            let cloud_name = &source.name;
            
            for bucket in &source.buckets {
                let bucket_name = &bucket.name;
                
                // 添加到 branch_cloud_source
                if !branch_cloud_source.contains_key(bucket_name) {
                    branch_cloud_source.insert(bucket_name.to_string(), HashMap::new());
                }
                
                branch_cloud_source.entry(bucket_name.to_string()).or_insert(HashMap::new()).insert(cloud_name.clone(), bucket.clone());
            }
            
            self.cloud_source_map.insert(cloud_name.clone(), source.clone());
        }
        
        self.branch_cloud_source = branch_cloud_source;
        
        // 加载魔法参数
        let mut cloud_magics_map = HashMap::new();
        for magic in &local_config.cloud_magics {
            cloud_magics_map.insert(magic.name.clone(), magic.clone());
        }
        for magic in &config.cloud_magics {
            cloud_magics_map.insert(magic.name.clone(), magic.clone());
        }
        self.cloud_magics_map = cloud_magics_map;
    }

    pub fn get_bucket_from_source(&self, source: &Value, bucket: &str) -> XResult<Value> {
        if let Some(buckets) = source["bucketMap"].as_object() {
            if let Some(bucket_source) = buckets.get(bucket) {
                return Ok(bucket_source.clone());
            }
        }
        Err(XError::BucketNotFound(bucket.to_string()))
    }

    pub fn is_xclouder(&self, key: &str) -> bool {
        self.take_cloud(key).is_some()
    }

    pub fn take_cloud(&self, key: &str) -> Option<String> {
        if key.starts_with("/_") {
            let key = &key[1..];
            return self.take_cloud(key);
        }
        
        if key.starts_with('_') {
            if let Some(idx) = key.find('/') {
                return Some(key[..idx].to_string());
            }
        }
        None
    }

    pub fn simple_key(&self, key: &str) -> String {
        if key.starts_with("/_") {
            return self.simple_key(&key[1..]);
        }
        
        if key.starts_with('_') {
            if let Some(idx) = key.find('/') {
                return key[idx + 1..].to_string();
            }
        }
        key.to_string()
    }

    async fn try_switch_domain(&self, bucket_source: &BucketSource) -> XResult<&BucketSource> {
        // 获取所有可用的备用名
        let sources = self.feedback_bucket_sources(bucket_source, &[bucket_source])?;
        
        // 检查每个域名的可用性
        for source in sources {
          if let Some(domain) = &source.domain {
            if let Ok(true) = self.native.check_dns(domain).await {
                return Ok(source);
            }
          }
        }
        
        Err(XError::NetworkError("No available domain".to_string()))
    }

    fn feedback_bucket_sources(&self, bucket_source: &BucketSource, ignore: &[&BucketSource]) -> XResult<Vec<&BucketSource>> {
        let mut sources = Vec::new();
        let mut current = bucket_source;
        
        while let Some(fallback) = &current.fallback {
            let (cloud_name, bucket_name) = self.parse_fallback(&fallback);
            let bucket_name = bucket_name.unwrap_or_else(|| &current.name);
            
            if let Ok(source) = self.current_bucket_source(bucket_name, cloud_name, false) {
                if !ignore.contains(&source) && !sources.contains(&source) {
                    sources.push(source);
                    current = source;
                    continue;
                }
            }
            break;
        }
        
        Ok(sources)
    }
    fn parse_fallback<'b>(&self, fallback: &'b str) -> (&'b str, Option<&'b str>) {
        fallback.split_once('.')
            .map(|(cloud, bucket)| (cloud, Some(bucket)))
            .unwrap_or((fallback, None))
    }

    pub fn current_magics(&self, magics: &[&str]) -> XResult<Vec<&CloudMagic>> {
        let magics = magics.iter().map(|magic| self.cloud_magics_map.get(*magic).ok_or_else(|| XError::InvalidConfig)).collect::<Result<Vec<_>, _>>()?;
        Ok(magics)
    }

    fn when_percent(scale: i64) -> bool {
        use rand::Rng;
        rand::thread_rng().gen_range(0..100) < scale
    }
}

#[derive(Clone)]
pub struct UploadOpts<'a> {
    pub bucket_source: &'a BucketSource,
    pub bucket: String,
    pub filename: String,
    pub file_path: String,
    pub key: String,
    pub on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
    pub up_id: i64,
    pub disable_retry: bool,
    pub manual_retry: bool,
}

impl<'a> Serialize for UploadOpts<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("UploadParams", 8)?;
        state.serialize_field("bucket_source", &self.bucket_source)?;
        state.serialize_field("bucket", &self.bucket)?;
        state.serialize_field("filename", &self.filename)?;
        state.serialize_field("file_path", &self.file_path)?;
        state.serialize_field("key", &self.key)?;
        state.serialize_field("up_id", &self.up_id)?;
        state.serialize_field("disable_retry", &self.disable_retry)?;
        state.serialize_field("manual_retry", &self.manual_retry)?;
        state.end()
    }
}

impl<'a> std::fmt::Debug for UploadOpts<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadParams")
            .field("bucket_source", &self.bucket_source)
            .field("bucket", &self.bucket)
            .field("filename", &self.filename)
            .field("file_path", &self.file_path)
            .field("key", &self.key)
            .field("on_progress", &self.on_progress.as_ref().map(|_| "Fn(f32)"))
            .field("up_id", &self.up_id)
            .field("disable_retry", &self.disable_retry)
            .field("manual_retry", &self.manual_retry)
            .finish()
    }
} 