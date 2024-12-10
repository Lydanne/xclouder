use std::collections::HashMap;

use crate::config::{BucketSource, CloudMagic, CloudSource};

pub type BucketSourceMap<'a> = HashMap<&'a str, BucketSource>; // {cloud_name: BucketSource}

pub type CloudSourceMap<'a> = HashMap<&'a str, CloudSource>; // {cloud_name: CloudSource}

pub type CloudMagicMap<'a> = HashMap<&'a str, CloudMagic>; // {cloud_name: CloudMagic}