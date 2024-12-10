use chrono::{DateTime, Utc};
use rand::Rng;

pub fn shot_unique() -> String {
    let base_time = 1732636800; // 2024-11-27 00:00:00
    let now = Utc::now().timestamp();
    let rel_now = now - base_time;
    let rand_num = rand::thread_rng().gen_range(0..10);
    let id = (rel_now * 100 + rand_num) * 10000 + get_counter();
    format!("{:x}", id)
}

// 计数器，用于生成唯一ID
static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn get_counter() -> i64 {
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as i64 % 10000
}

pub fn fill_name(file_path: &str, openid: &str) -> String {
    let ext = extract_ext(file_path);
    format!("{}/{}{}", openid, shot_unique(), ext)
}

pub fn fix_name(file_path: &str) -> String {
    file_path.replace("_tos", "_fix")
        .replace("_cos", "_fix")
        .replace("_oss", "_fix")
}

fn extract_ext(file_path: &str) -> String {
    if let Some(dot_pos) = file_path.rfind('.') {
        file_path[dot_pos..].to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shot_unique() {
        let id1 = shot_unique();
        let id2 = shot_unique();
        assert_ne!(id1, id2);
        assert!(id1.len() > 0);
        assert!(id2.len() > 0);
    }

    #[test]
    fn test_fill_name() {
        let name = fill_name("test.jpg", "test_user");
        assert!(name.ends_with(".jpg"));
        assert!(name.starts_with("test_user/"));
        
        let name2 = fill_name("test.png", "other_user");
        assert!(name2.ends_with(".png"));
        assert!(name2.starts_with("other_user/"));
        assert_ne!(name, name2);
    }

    #[test]
    fn test_fix_name() {
        assert_eq!(fix_name("_cos/test.jpg"), "_fix/test.jpg");
        assert_eq!(fix_name("_tos/test.jpg"), "_fix/test.jpg");
        assert_eq!(fix_name("_oss/test.jpg"), "_fix/test.jpg");
        assert_eq!(fix_name("test.jpg"), "test.jpg");
    }

    #[test]
    fn test_extract_ext() {
        assert_eq!(extract_ext("test.jpg"), ".jpg");
        assert_eq!(extract_ext("test"), "");
        assert_eq!(extract_ext("test.tar.gz"), ".gz");
        assert_eq!(extract_ext(".gitignore"), ".gitignore");
    }
} 