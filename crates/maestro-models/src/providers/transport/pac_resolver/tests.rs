//! Authored scripts observed through the retained interpreter.

#[test]
fn pac_domain_suffix_keeps_empty_and_case_boundaries() {
    crate::tests::runtime::run(async {
        let resolver = super::Resolver::new("function FindProxyForURL(){return JSON.stringify([dnsDomainIs('www.example.com','.example.com'),dnsDomainIs('example.com','.example.com'),dnsDomainIs('x',''),dnsDomainIs('',''),dnsDomainIs('A','a'),dnsDomainIs(null,'null'),dnsDomainIs(12,'2'),dnsDomainIs(true,'true'),dnsDomainIs({},'Object]'),dnsDomainIs(['a','b'],'b'),dnsDomainIs(undefined,'undefined')]);}".into()).await.unwrap();
        assert_eq!(
            resolver.resolve("https://example.com/").await.unwrap(),
            "[true,false,false,true,false,true,true,true,true,true,true]"
        );
        resolver.close().await.unwrap();
        assert_eq!(expression("[dnsDomainIs(),...[null,'plain',123,true,{},['a','b']].map(x=>dnsDomainIs(x,'plain'))]").await,
            "[true,false,true,false,false,false,false]");
    });
}

/// Invoke an authored expression and join its interpreter before returning the result.
async fn expression(text: &str) -> String {
    let resolver = super::Resolver::new(format!(
        "function FindProxyForURL(url,host){{return JSON.stringify({text});}}"
    ))
    .await
    .unwrap();
    let result = resolver
        .resolve("https://example.com:8443/a%20b?q=1&q=2")
        .await
        .unwrap();
    resolver.close().await.unwrap();
    result
}

#[test]
fn pac_domain_levels_count_only_literal_dots() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("['','a','.a.','a..b','...','雪.例',null,undefined,12,true,{},['a.b']].map(x=>dnsDomainLevels(x))").await,
            "[0,0,2,2,3,1,0,0,0,0,0,1]");
    });
}

#[test]
fn pac_plain_hosts_distinguish_dots_from_colons() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("['','host','::1','host.name','host.','127.0.0.1',null,undefined,12,true,{},['a.b']].map(x=>isPlainHostName(x))").await,
            "[true,true,true,false,false,false,true,true,true,true,true,false]");
    });
}

#[test]
fn pac_local_domain_compares_complete_label_prefixes() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("[['www','www.example.com'],['www.example','www.example.com'],['www.example.com','www.example.com'],['www.other','www.example.com'],['ww','www.example.com'],['www.example.com.extra','www.example.com'],['',''],['','x']].map(x=>localHostOrDomainIs(...x))").await,
            "[true,true,true,false,false,false,true,false]");
        assert_eq!(expression("[null,undefined,12,true,{},[]].map(x=>{try{return localHostOrDomainIs(x,'host')}catch(e){return 'error'}})").await,
            "[\"error\",\"error\",\"error\",\"error\",\"error\",\"error\"]");
        assert_eq!(expression("[[],...[null,'plain',123,true,{},['a','b']].map(x=>[x,'plain.example'])].map(args=>{try{return localHostOrDomainIs(...args)}catch(e){return 'error'}})").await,
            "[\"error\",\"error\",true,\"error\",\"error\",\"error\",\"error\"]");
    });
}

#[test]
fn pac_shell_patterns_keep_wildcards_and_regex_forms() {
    crate::tests::runtime::run(async {
        assert_eq!(expression(r"[['abc','a*'],['abc','a?c'],['a.c','a.c'],['abc','a.c'],['b','[ab]'],['x','x|y'],['y','x|y'],['foo','^foo$'],['a\nb','*'],['abc','[']].map(([text,pattern])=>{try{return shExpMatch(text,pattern)}catch(e){return 'error'}})").await,
            "[true,true,true,false,true,true,true,true,false,\"error\"]");
    });
}

#[test]
fn pac_dns_returns_ipv4_or_null() {
    crate::tests::runtime::run(async {
        assert_eq!(
            expression(
                "['127.0.0.1','localhost','::1','missing.invalid'].map(host=>dnsResolve(host))"
            )
            .await,
            "[\"127.0.0.1\",\"127.0.0.1\",null,null]"
        );
    });
}

#[test]
fn pac_dns_resolvability_reports_success_and_failure() {
    crate::tests::runtime::run(async {
        assert_eq!(
            expression(
                "['127.0.0.1','localhost','::1','missing.invalid'].map(host=>isResolvable(host))"
            )
            .await,
            "[true,true,false,false]"
        );
    });
}

#[test]
fn pac_network_membership_uses_resolved_ipv4() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("[['127.0.0.1','127.0.0.0','255.0.0.0'],['127.0.0.1','128.0.0.0','8'],['localhost','127.0.0.1','32'],['missing.invalid','0','0'],['::1','0','0'],['127.0.0.1','bad','8'],['127.0.0.1','127','bad'],['127.0.0.1','0','0.0.0.0'],['127.0.0.1','0','0']].map(x=>isInNet(...x))").await,
            "[true,false,true,false,false,false,false,true,true]");
    });
}

#[test]
fn pac_network_patterns_keep_accepted_address_notations() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("['127.0.0.1','0x7f.0.0.1','0177.0.0.1','127.1','127.0.1','2130706433','127.0.0.1/32'].map(net=>isInNet('127.0.0.1',net))").await,
            "[true,true,true,true,true,true,true]");
        assert_eq!(expression("['255.255.255.255','32','32suffix','255.0.255.0','33','bogus'].map(mask=>isInNet('127.0.0.1','127.0.0.1',mask))").await,
            "[true,true,true,false,false,false]");
        assert_eq!(expression("[['127','8'],['0x7f000000','8'],['0177.0.0.0','8'],['１２７.0.0.1','32'],['%31%32%37.0.0.1','32']].map(([net,mask])=>isInNet('127.0.0.1',net,mask))").await,
            "[false,true,true,false,false]");
        assert_eq!(
            expression("isInNet('127.0.0.1','127.0.0.1',{toString(){throw Error('bad mask')}})")
                .await,
            "false"
        );
    });
}

#[test]
fn pac_time_ranges_keep_each_arity_and_endpoint() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("[timeRange(0,24),timeRange(0,24,'GMT'),timeRange('0suffix','24hours'),timeRange(24,0),timeRange(),timeRange(1,2,3),timeRange('x'),timeRange(0,0,23,59),timeRange(0,0,0,23,59,59)]").await,
            "[true,true,true,false,false,false,false,true,true]");
    });
}

#[test]
fn pac_weekday_ranges_wrap_and_select_timezone() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("[weekdayRange('SUN','SAT'),weekdayRange('SAT','FRI','GMT'),weekdayRange('bad'),weekdayRange('mon'),weekdayRange('SUN','bad')===weekdayRange('SUN'),weekdayRange('SUN','GMT')===weekdayRange('SUN','GMT','GMT')]").await,
            "[true,true,false,false,true,true]");
        assert_eq!(expression("['SUN','MON','TUE','WED','THU','FRI','SAT'].filter(w=>weekdayRange(w,'GMT')).length").await, "1");
    });
}

#[test]
fn pac_date_ranges_match_documented_calendar_forms() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("[dateRange(1,31),dateRange('JAN','DEC'),dateRange(1900,3000),dateRange(1,'JAN',31,'DEC'),dateRange('JAN',1900,'DEC',3000),dateRange(1,'JAN',1900,31,'DEC',3000),dateRange(1,31,'GMT'),dateRange('bad'),dateRange()]").await,
            "[true,true,true,true,true,true,true,false,false]");
        assert_eq!(expression("(()=>{let d=new Date(),m=['JAN','FEB','MAR','APR','MAY','JUN','JUL','AUG','SEP','OCT','NOV','DEC'][d.getMonth()];return [dateRange(d.getDate()),dateRange(m),dateRange(d.getFullYear()),dateRange(d.getDate(),m),dateRange(d.getDate(),m,d.getFullYear())]})()").await,
            "[true,true,true,true,true]");
    });
}

#[test]
fn pac_interface_fallback_keeps_order_and_loopback_default() {
    let select = |observations: &[(&str, &str)]| {
        super::ip::select(
            observations
                .iter()
                .map(|(name, ip)| (*name, ip.parse().unwrap())),
        )
        .to_string()
    };
    assert_eq!(
        select(&[("A", "::1"), ("B", "10.0.0.1"), ("A", "169.254.1.2")]),
        "169.254.1.2"
    );
    assert_eq!(
        select(&[("B", "10.0.0.1"), ("A", "::1"), ("A", "169.254.1.2")]),
        "10.0.0.1"
    );
    assert_eq!(
        select(&[
            ("v6", "::1"),
            ("z-first", "127.2.3.4"),
            ("z-first", "::1"),
            ("z-first", "169.254.1.2"),
            ("z-first", "10.0.0.2"),
            ("a-second", "10.0.0.1")
        ]),
        "169.254.1.2"
    );
    assert_eq!(
        select(&[
            ("first", "10.0.0.2"),
            ("first", "169.254.1.2"),
            ("second", "10.0.0.1")
        ]),
        "10.0.0.2"
    );
    assert_eq!(select(&[("lo", "127.2.3.4"), ("v6", "::1")]), "127.0.0.1");
    assert_eq!(select(&[]), "127.0.0.1");
}

#[test]
fn pac_local_address_needs_no_answering_endpoint() {
    let endpoint = std::net::UdpSocket::bind("127.0.0.1:0").expect("reserve a UDP endpoint");

    assert_eq!(
        super::my_ip_address::address(endpoint.local_addr().unwrap()).unwrap(),
        std::net::Ipv4Addr::LOCALHOST
    );
}

#[test]
fn pac_local_address_falls_back_to_native_interfaces() {
    let endpoint = "[::1]:53".parse().expect("an IPv6 endpoint");
    let expected = super::ip::address().expect("read native interfaces");

    assert_eq!(super::my_ip_address::address(endpoint).unwrap(), expected);
}

mod alerts;
mod calendar;
mod cancellation;

#[test]
fn pac_helpers_are_available_to_authored_scripts() {
    crate::tests::runtime::run(async {
        let resolver = super::Resolver::new("let calls=0;function nested(host){return [dnsResolve(host),isResolvable(host),isInNet(host,'127.0.0.0','8')]}function FindProxyForURL(){return JSON.stringify([++calls,nested('localhost'),[alert,dateRange,dnsDomainIs,dnsDomainLevels,dnsResolve,isInNet,isPlainHostName,isResolvable,localHostOrDomainIs,myIpAddress,shExpMatch,timeRange,weekdayRange].map(fn=>fn.length)])}".into()).await.unwrap();
        for count in 1..=2 {
            assert_eq!(
                resolver.resolve("https://example.com").await.unwrap(),
                format!("[{count},[\"127.0.0.1\",true,true],[0,0,2,1,1,3,1,1,2,0,2,0,3]]")
            );
        }
        resolver.close().await.unwrap();
        let address: String = serde_json::from_str(&expression("myIpAddress()").await).unwrap();
        let ip = address.parse::<std::net::Ipv4Addr>().unwrap();
        assert!(
            ip == std::net::Ipv4Addr::LOCALHOST
                || if_addrs::get_if_addrs()
                    .unwrap()
                    .iter()
                    .any(|interface| interface.ip() == ip)
        );
    });
}

#[test]
fn pac_dns_helpers_share_one_lookup_operation() {
    crate::tests::runtime::run(async {
        assert_eq!(expression("['localhost','missing.invalid','::1'].map(host=>[dnsResolve(host)!==null,isResolvable(host),isInNet(host,'0','0')])").await, "[[true,true,true],[false,false,false],[false,false,false]]");
    });
}

mod settlement;
