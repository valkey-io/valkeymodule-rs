use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

lazy_static::lazy_static! {
    pub(crate) static ref API_VERSION_MAPPING: HashMap<String, usize> = HashMap::from([
        ("RedisModule_AddPostNotificationJob".to_string(), 70200),
        ("RedisModule_SetCommandACLCategories".to_string(), 70200),
        ("RedisModule_GetOpenKeyModesAll".to_string(), 70200),
        ("RedisModule_CallReplyPromiseSetUnblockHandler".to_string(), 70200),
        ("RedisModule_CallReplyPromiseAbort".to_string(), 70200),
        ("RedisModule_Microseconds".to_string(), 70200),
        ("RedisModule_CachedMicroseconds".to_string(), 70200),
        ("RedisModule_RegisterAuthCallback".to_string(), 70200),
        ("RedisModule_BlockClientOnKeysWithFlags".to_string(), 70200),
        ("RedisModule_GetModuleOptionsAll".to_string(), 70200),
        ("RedisModule_BlockClientGetPrivateData".to_string(), 70200),
        ("RedisModule_BlockClientSetPrivateData".to_string(), 70200),
        ("RedisModule_BlockClientOnAuth".to_string(), 70200),
        ("RedisModule_ACLAddLogEntryByUserName".to_string(), 70200),
        ("RedisModule_GetCommand".to_string(), 70000),
        ("RedisModule_SetCommandInfo".to_string(), 70000),
        ("RedisModule_AddACLCategory".to_string(), 80000),
    ]);

    pub(crate) static ref API_OLDEST_VERSION: usize = 60000;
    pub(crate) static ref ALL_VERSIONS: Vec<(usize, String)> = vec![
        (60000, "min-redis-compatibility-version-6-0".to_string()),
        (60200, "min-redis-compatibility-version-6-2".to_string()),
        (70000, "min-redis-compatibility-version-7-0".to_string()),
        (70200, "min-redis-compatibility-version-7-2".to_string()),
        (80000, "min-valkey-compatibility-version-8-0".to_string()),
        (90000, "min-valkey-compatibility-version-9-0".to_string()),
    ];
}

pub(crate) fn get_feature_flags(
    min_required_version: usize,
) -> (Vec<TokenStream>, TokenStream, Vec<TokenStream>) {
    let all_lower_versions: Vec<&str> = ALL_VERSIONS
        .iter()
        .filter_map(|(v, s)| {
            if *v < min_required_version {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect();
    let required_feature = ALL_VERSIONS
        .iter()
        .find_map(|(v, s)| (*v == min_required_version).then_some(s.as_str()))
        .expect("every mapped API version must have a compatibility feature");
    let all_higher_versions: Vec<&str> = ALL_VERSIONS
        .iter()
        .filter_map(|(v, s)| (*v > min_required_version).then_some(s.as_str()))
        .collect();
    (
        all_lower_versions
            .into_iter()
            .map(|s| quote!(feature = #s).into())
            .collect(),
        quote!(feature = #required_feature).into(),
        all_higher_versions
            .into_iter()
            .map(|s| quote!(feature = #s).into())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::get_feature_flags;

    #[test]
    fn redis_7_2_uses_its_existing_minimum_feature() {
        let (_, required, _) = get_feature_flags(70200);
        assert_eq!(
            required.to_string(),
            "feature = \"min-redis-compatibility-version-7-2\""
        );
    }

    #[test]
    fn valkey_8_uses_its_existing_minimum_feature() {
        let (_, required, _) = get_feature_flags(80000);
        assert_eq!(
            required.to_string(),
            "feature = \"min-valkey-compatibility-version-8-0\""
        );
    }

    #[test]
    fn valkey_9_uses_its_existing_minimum_feature() {
        let (_, required, _) = get_feature_flags(90000);
        assert_eq!(
            required.to_string(),
            "feature = \"min-valkey-compatibility-version-9-0\""
        );
    }

    #[test]
    fn lower_features_exclude_the_api_minimum_feature() {
        let (lower_features, required, _) = get_feature_flags(80000);
        let lower_features: Vec<String> = lower_features.iter().map(ToString::to_string).collect();

        assert_eq!(
            required.to_string(),
            "feature = \"min-valkey-compatibility-version-8-0\""
        );
        assert_eq!(
            lower_features,
            vec![
                "feature = \"min-redis-compatibility-version-6-0\"",
                "feature = \"min-redis-compatibility-version-6-2\"",
                "feature = \"min-redis-compatibility-version-7-0\"",
                "feature = \"min-redis-compatibility-version-7-2\"",
            ]
        );
    }

    #[test]
    fn valkey_9_selection_has_a_direct_valkey_8_gate() {
        let (_, required, newer_features) = get_feature_flags(80000);
        let newer_features: Vec<String> = newer_features.iter().map(ToString::to_string).collect();

        assert_eq!(
            required.to_string(),
            "feature = \"min-valkey-compatibility-version-8-0\""
        );
        assert_eq!(
            newer_features,
            vec!["feature = \"min-valkey-compatibility-version-9-0\""]
        );
    }
}
