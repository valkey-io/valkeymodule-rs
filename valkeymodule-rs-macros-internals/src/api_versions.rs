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
) -> (Vec<TokenStream>, Vec<TokenStream>) {
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
    let all_upper_versions: Vec<&str> = ALL_VERSIONS
        .iter()
        .filter_map(|(v, s)| {
            if *v >= min_required_version {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect();
    (
        all_lower_versions
            .into_iter()
            .map(|s| quote!(feature = #s).into())
            .collect(),
        all_upper_versions
            .into_iter()
            .map(|s| quote!(feature = #s).into())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::get_feature_flags;

    #[test]
    fn partitions_features_at_the_minimum_required_version() {
        // Keep expectations independent of ALL_VERSIONS so missing or misnamed
        // features are caught as well as incorrect boundary comparisons.
        let features = [
            "feature = \"min-redis-compatibility-version-6-0\"",
            "feature = \"min-redis-compatibility-version-6-2\"",
            "feature = \"min-redis-compatibility-version-7-0\"",
            "feature = \"min-redis-compatibility-version-7-2\"",
            "feature = \"min-valkey-compatibility-version-8-0\"",
            "feature = \"min-valkey-compatibility-version-9-0\"",
        ];

        // The split index is the number of features strictly below the minimum.
        // Versions use major * 10000 + minor * 100 + patch (e.g. 70200 = 7.2.0).
        for (minimum, split) in [
            (59999, 0), // Below the oldest version: every feature meets the minimum.
            (60000, 0), // Exact matches belong to the upper group, including the oldest.
            (60200, 1),
            (70000, 2),
            (70100, 3), // Between known versions: Redis 7.2 is the first eligible feature.
            (70200, 3),
            (80000, 4),
            (90000, 5),
            (90001, 6), // Above the newest version: no feature meets the minimum.
        ] {
            let (lower, upper) = get_feature_flags(minimum);
            let lower: Vec<String> = lower.iter().map(ToString::to_string).collect();
            let upper: Vec<String> = upper.iter().map(ToString::to_string).collect();

            // Check both complete groups to catch omissions, duplicates, and
            // off-by-one errors that place the minimum in the wrong group.
            assert_eq!(lower, features[..split], "lower features for {minimum}");
            assert_eq!(upper, features[split..], "upper features for {minimum}");
        }
    }
}
