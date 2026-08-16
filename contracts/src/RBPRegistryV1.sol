// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.24;

/// @title Rebootable Bootstrap Protocol registry v1
/// @notice Permissionless, immutable, log-only rendezvous for signed peer records.
/// @dev This contract deliberately has no owner, storage, upgrade, pause, or withdrawal path.
contract RBPRegistryV1 {
    uint32 public constant VERSION = 1;
    uint32 public constant MAX_TTL = 90 days;
    uint32 public constant MAX_RECORD_BYTES = 4096;

    error InvalidTTL();
    error RecordTooLarge();

    event PeerAnnounced(
        bytes32 indexed namespace, uint32 indexed recordType, uint64 validUntil, bytes peerRecord
    );

    /// @notice Publishes a bounded-lifetime, application-namespaced signed peer record.
    /// @param namespace Application/network isolation identifier.
    /// @param recordType RBP peer-record codec identifier.
    /// @param ttl Lifetime in seconds, bounded by `MAX_TTL`.
    /// @param peerRecord Raw self-authenticating peer-record bytes.
    function announce(bytes32 namespace, uint32 recordType, uint32 ttl, bytes calldata peerRecord)
        external
    {
        if (ttl == 0 || ttl > MAX_TTL) revert InvalidTTL();
        if (peerRecord.length == 0 || peerRecord.length > MAX_RECORD_BYTES) {
            revert RecordTooLarge();
        }

        emit PeerAnnounced(namespace, recordType, uint64(block.timestamp) + uint64(ttl), peerRecord);
    }
}

