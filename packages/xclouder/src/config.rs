use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "cloudSource")]
    pub cloud_source: Vec<CloudSource>,
    #[serde(rename = "cloudMagics")]
    pub cloud_magics: Vec<CloudMagic>,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct CloudSource {
    pub name: String,
    pub cloud: Option<String>,
    pub grayscale: Option<i64>,
    pub buckets: Vec<BucketSource>,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct BucketSource {
    pub name: String,
    pub domain: Option<String>,
    #[serde(rename = "cdnDomain")]
    pub cdn_domain: Option<String>,
    pub fallback: Option<String>,

    pub cloud: Option<String>,
    #[serde(rename = "cloudName")]
    pub cloud_name: Option<String>,
    pub grayscale: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMagic {
    pub name: String,
    #[serde(rename = "cloudCfg")]
    pub cloud_cfg: HashMap<String, String>,
}

impl CloudMagic {
    pub fn get_magic(&self, cloud: &str) -> Option<&str> {
        self.cloud_cfg.get(cloud).map(|s| s.as_str())
    }
}

impl Config {
    pub fn from_json(value: Value) -> serde_json::Result<Self> {
        serde_json::from_value(value)
    }

    pub fn get_bucket(&self, cloud_name: &str, bucket_name: &str) -> Option<&BucketSource> {
        self.cloud_source
            .iter()
            .find(|source| source.name == cloud_name)?
            .buckets
            .iter()
            .find(|bucket| bucket.name == bucket_name)
    }

    pub fn get_cloud_source(&self, name: &str) -> Option<&CloudSource> {
        self.cloud_source
            .iter()
            .find(|source| source.name == name)
    }

    pub fn get_magic(&self, name: &str) -> Option<&CloudMagic> {
        self.cloud_magics
            .iter()
            .find(|magic| magic.name == name)
    }

    pub fn get_magic_cfg(&self, magic_name: &str, cloud: &str) -> Option<&str> {
        self.get_magic(magic_name)?
            .cloud_cfg
            .get(cloud)
            .map(|s| s.as_str())
    }

    pub fn resolve_fallback(&self, bucket_source: &CloudSource, bucket_name: &str) -> Option<(&CloudSource, &BucketSource)> {
        let bucket = bucket_source.buckets
            .iter()
            .find(|b| b.name == bucket_name)?;

        if let Some(fallback) = &bucket.fallback {
            let (cloud_name, bucket_name) = fallback.split_once('.')?;
            let source = self.get_cloud_source(cloud_name)?;
            let bucket = source.buckets
                .iter()
                .find(|b| b.name == bucket_name)?;
            Some((source, bucket))
        } else {
            None
        }
    }

    pub fn get_bucket_domain(&self, cloud_name: &str, bucket_name: &str) -> Option<&str> {
        self.get_bucket(cloud_name, bucket_name)
            .and_then(|bucket| bucket.domain.as_deref())
    }

    pub fn get_bucket_cdn_domain(&self, cloud_name: &str, bucket_name: &str) -> Option<&str> {
        self.get_bucket(cloud_name, bucket_name)
            .and_then(|bucket| bucket.cdn_domain.as_deref())
    }

    pub fn get_cloud_type(&self, cloud_name: &str) -> Option<&str> {
        self.get_cloud_source(cloud_name)
            .and_then(|source| source.cloud.as_deref())
    }

    pub fn get_grayscale(&self, cloud_name: &str) -> Option<i64> {
        self.get_cloud_source(cloud_name)
            .and_then(|source| source.grayscale)
    }

    pub fn merge(&mut self, other: &Config) {
        // 合并云源配置
        for source in &other.cloud_source {
            if let Some(existing) = self.cloud_source.iter_mut()
                .find(|s| s.name == source.name) {
                existing.cloud = source.cloud.clone();
                existing.grayscale = source.grayscale;
                
                for bucket in &source.buckets {
                    if let Some(existing_bucket) = existing.buckets.iter_mut()
                        .find(|b| b.name == bucket.name) {
                        existing_bucket.domain = bucket.domain.clone();
                        existing_bucket.cdn_domain = bucket.cdn_domain.clone();
                        existing_bucket.fallback = bucket.fallback.clone();
                    } else {
                        existing.buckets.push(bucket.clone());
                    }
                }
            } else {
                self.cloud_source.push(source.clone());
            }
        }

        // 合并魔法参数配置
        for magic in &other.cloud_magics {
            if let Some(existing) = self.cloud_magics.iter_mut()
                .find(|m| m.name == magic.name) {
                existing.cloud_cfg.extend(magic.cloud_cfg.clone());
            } else {
                self.cloud_magics.push(magic.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let json = r#"{
            "cloudSource": [
                {
                    "name": "_cos",
                    "cloud": "cos",
                    "buckets": [
                        {
                            "name": "img",
                            "domain": "img-1302562365.cos.ap-beijing.myqcloud.com",
                            "cdnDomain": "img.banjixiaoguanjia.com",
                            "fallback": "_cos.backup-img"
                        },
                        {
                            "name": "backup-img",
                            "domain": "img-1302562365.cos.ap-beijing.myqcloud.com",
                            "cdnDomain": "img.banjixiaoguanjia.com"
                        }
                    ]
                }
            ],
            "cloudMagics": [
                {
                    "name": "thumbnail_200",
                    "cloudCfg": {
                        "cos": "imageMogr2/thumbnail/200>/format/jpg",
                        "oss": "x-oss-process=image/resize,w_200,h_200,m_fill/auto-orient,1/interlace,1/format,jpg",
                        "tos": ""
                    }
                }
            ]
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        
        assert_eq!(config.cloud_source[0].name, "_cos");
        assert_eq!(config.cloud_source[0].buckets[0].name, "img");
        assert_eq!(config.cloud_magics[0].name, "thumbnail_200");
        
        // 测试获取 bucket
        let bucket = config.get_bucket("_cos", "img").unwrap();
        assert_eq!(bucket.domain.as_deref().unwrap(), "img-1302562365.cos.ap-beijing.myqcloud.com");
        
        // 测试获取 magic 配置
        let cfg = config.get_magic_cfg("thumbnail_200", "cos").unwrap();
        assert_eq!(cfg, "imageMogr2/thumbnail/200>/format/jpg");
        
        // 测试解析 fallback
        let source = config.get_cloud_source("_cos").unwrap();
        let (fallback_source, fallback_bucket) = config.resolve_fallback(source, "img").unwrap();
        assert_eq!(fallback_source.name, "_cos");
        assert_eq!(fallback_bucket.name, "backup-img");
    }
} 