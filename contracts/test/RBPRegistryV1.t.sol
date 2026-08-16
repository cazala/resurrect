// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { RBPRegistryV1 } from "../src/RBPRegistryV1.sol";
import { TestBase } from "./TestBase.sol";

contract RBPRegistryV1Test is TestBase {
    RBPRegistryV1 internal registry;

    event PeerAnnounced(
        bytes32 indexed namespace, uint32 indexed recordType, uint64 validUntil, bytes peerRecord
    );

    function setUp() public {
        registry = new RBPRegistryV1();
    }

    function testConstants() public view {
        assertEq(registry.VERSION(), 1);
        assertEq(registry.MAX_TTL(), 90 days);
        assertEq(registry.MAX_RECORD_BYTES(), 4096);
    }

    function testTtlZeroReverts() public {
        vm.expectRevert(RBPRegistryV1.InvalidTTL.selector);
        registry.announce(bytes32(uint256(1)), 2, 0, hex"01");
    }

    function testTtlAboveMaximumReverts() public {
        uint32 invalidTtl = registry.MAX_TTL() + 1;
        vm.expectRevert(RBPRegistryV1.InvalidTTL.selector);
        registry.announce(bytes32(uint256(1)), 2, invalidTtl, hex"01");
    }

    function testEmptyRecordReverts() public {
        vm.expectRevert(RBPRegistryV1.RecordTooLarge.selector);
        registry.announce(bytes32(uint256(1)), 2, 1, "");
    }

    function testOversizedRecordReverts() public {
        vm.expectRevert(RBPRegistryV1.RecordTooLarge.selector);
        registry.announce(bytes32(uint256(1)), 2, 1, new bytes(4097));
    }

    function testValidAnnouncementEmitsContractDerivedExpiry() public {
        bytes32 namespace = keccak256("rbp:test:1");
        bytes memory peerRecord = hex"010203";
        vm.warp(1_700_000_000);
        vm.expectEmit(true, true, false, true, address(registry));
        emit PeerAnnounced(namespace, 2, 1_700_000_600, peerRecord);
        registry.announce(namespace, 2, 600, peerRecord);
    }

    function testAnyAddressCanAnnounce() public {
        address arbitraryCaller = address(0xBEEF);
        vm.prank(arbitraryCaller);
        registry.announce(bytes32(uint256(1)), 2, 1, hex"01");
    }

    function testNoOwnerAdminPauseOrUpgradeSurfaceExists() public {
        (bool ownerOk,) = address(registry).call(abi.encodeWithSignature("owner()"));
        (bool pauseOk,) = address(registry).call(abi.encodeWithSignature("pause()"));
        (bool upgradeOk,) =
            address(registry).call(abi.encodeWithSignature("upgradeTo(address)", address(1)));
        (bool transferOk,) = address(registry)
            .call(abi.encodeWithSignature("transferOwnership(address)", address(1)));
        assertFalse(ownerOk);
        assertFalse(pauseOk);
        assertFalse(upgradeOk);
        assertFalse(transferOk);
    }

    function testFuzzValidBoundsAlwaysSucceed(
        bytes32 namespace,
        uint32 recordType,
        uint32 ttlSeed,
        bytes calldata input
    ) public {
        uint32 ttl = uint32(uint256(ttlSeed) % registry.MAX_TTL()) + 1;
        uint256 size = (input.length % registry.MAX_RECORD_BYTES()) + 1;
        bytes memory record = new bytes(size);
        registry.announce(namespace, recordType, ttl, record);
    }

    function testFuzzNoStorageIsWritten(
        bytes32 namespace,
        uint32 recordType,
        uint32 ttlSeed,
        bytes calldata input
    ) public {
        uint32 ttl = uint32(uint256(ttlSeed) % registry.MAX_TTL()) + 1;
        uint256 size = (input.length % 256) + 1;
        registry.announce(namespace, recordType, ttl, new bytes(size));
        for (uint256 slot; slot < 32; ++slot) {
            assertEq(uint256(vm.load(address(registry), bytes32(slot))), 0);
        }
    }
}
