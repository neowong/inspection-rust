#[cfg(test)]
mod tools_tests {
    // Import from the library
    use inspection_rust_lib::services;

    #[test]
    fn test_port_parsing() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Test individual port
        let result = rt.block_on(services::port_scanner::scan_ports("127.0.0.1", "22", 100));
        assert!(result.is_ok());
        let ports: Vec<_> = result.unwrap().into_iter().map(|r| r.port).collect();
        assert_eq!(ports, vec![22]);

        // Test multiple ports
        let result = rt.block_on(services::port_scanner::scan_ports("127.0.0.1", "22,80,443", 100));
        assert!(result.is_ok());
        let ports: Vec<_> = result.unwrap().into_iter().map(|r| r.port).collect();
        assert_eq!(ports, vec![22, 80, 443]);

        // Test port range
        let result = rt.block_on(services::port_scanner::scan_ports("127.0.0.1", "1-5", 100));
        assert!(result.is_ok());
        let ports: Vec<_> = result.unwrap().into_iter().map(|r| r.port).collect();
        assert_eq!(ports, vec![1, 2, 3, 4, 5]);

        // Test mixed
        let result = rt.block_on(services::port_scanner::scan_ports("127.0.0.1", "22,80-82", 100));
        assert!(result.is_ok());
        let ports: Vec<_> = result.unwrap().into_iter().map(|r| r.port).collect();
        assert_eq!(ports, vec![22, 80, 81, 82]);

        // Test invalid
        let result = rt.block_on(services::port_scanner::scan_ports("127.0.0.1", "", 100));
        assert!(result.is_err());
    }

    #[test]
    fn test_cidr_parsing_live_scanner() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // /30 subnet should produce 2 host IPs
        let result = rt.block_on(services::live_scanner::scan_subnet("192.168.1.0/30", 100));
        assert!(result.is_ok());
        let ips: Vec<_> = result.unwrap().into_iter().map(|r| r.ip).collect();
        assert_eq!(ips, vec!["192.168.1.1", "192.168.1.2"]);

        // /32 should produce 1 IP
        let result = rt.block_on(services::live_scanner::scan_subnet("10.0.0.1/32", 100));
        assert!(result.is_ok());
        let ips: Vec<_> = result.unwrap().into_iter().map(|r| r.ip).collect();
        assert_eq!(ips, vec!["10.0.0.1"]);

        // Invalid CIDR
        let result = rt.block_on(services::live_scanner::scan_subnet("not-a-cidr", 100));
        assert!(result.is_err());
    }

    #[test]
    fn test_snmp_packet_build() {
        let packet = services::snmp_checker::build_snmp_v2c_get("public", "1.3.6.1.2.1.1.1.0");
        assert!(packet.is_ok());
        let bytes = packet.unwrap();
        // Should start with 0x30 (SEQUENCE)
        assert_eq!(bytes[0], 0x30);
        // Should contain "public"
        let public_idx = bytes.windows(6).position(|w| w == b"public");
        assert!(public_idx.is_some());
    }

    #[test]
    fn test_snmp_packet_invalid_oid() {
        let result = services::snmp_checker::build_snmp_v2c_get("public", "not-an-oid");
        assert!(result.is_err());
    }

    #[test]
    fn test_web_url_normalization() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Just test that the function doesn't panic with various URLs
        let urls = vec![
            "https://www.baidu.com".to_string(),
            "http://example.com".to_string(),
            "github.com".to_string(),
        ];
        let results = rt.block_on(services::web_checker::check_urls(&urls, 5));
        assert_eq!(results.len(), 3);
        // First URL should work
        assert!(results.iter().all(|r| !r.url.is_empty()));
    }
}
