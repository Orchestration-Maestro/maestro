//! The login refresh region choice.

use aws_sdk_bedrockruntime::config::Region;

use super::login_region;

#[test]
fn login_profile_region_precedes_request_region() {
    let (europe, america) = (Region::new("eu-west-1"), Region::new("us-east-1"));
    assert_eq!(
        login_region(Some("eu-west-1"), Some(&america)),
        Some(europe.clone())
    );
    assert_eq!(
        login_region(Some("us-east-1"), Some(&europe)),
        Some(america)
    );
    assert_eq!(login_region(None, Some(&europe)), Some(europe));
}
