use alloy::sol;

sol! {
    /// Canonical RBP registry v1 ABI.
    #[sol(rpc)]
    interface RBPRegistryV1 {
        error InvalidTTL();
        error RecordTooLarge();

        event PeerAnnounced(
            bytes32 indexed namespace,
            uint32 indexed recordType,
            uint64 validUntil,
            bytes peerRecord
        );

        function VERSION() external view returns (uint32);
        function MAX_TTL() external view returns (uint32);
        function MAX_RECORD_BYTES() external view returns (uint32);
        function announce(
            bytes32 namespace,
            uint32 recordType,
            uint32 ttl,
            bytes calldata peerRecord
        ) external;
    }
}
