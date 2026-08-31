// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { ResurrectRegistryV1 } from "../src/ResurrectRegistryV1.sol";
import { InvariantBase } from "./TestBase.sol";

contract AnnouncementHandler {
    ResurrectRegistryV1 internal immutable registry;

    constructor(ResurrectRegistryV1 registry_) {
        registry = registry_;
    }

    function announce(bytes32 namespace, uint32 recordType, uint32 ttlSeed, bytes calldata input)
        external
    {
        uint32 ttl = uint32(uint256(ttlSeed) % registry.MAX_TTL()) + 1;
        uint256 size = (input.length % 512) + 1;
        registry.announce(namespace, recordType, ttl, new bytes(size));
    }
}

contract ResurrectRegistryV1InvariantTest is InvariantBase {
    ResurrectRegistryV1 internal registry;
    AnnouncementHandler internal handler;

    function setUp() public {
        registry = new ResurrectRegistryV1();
        handler = new AnnouncementHandler(registry);
    }

    function targetContracts() public view override returns (address[] memory targets) {
        targets = new address[](1);
        targets[0] = address(handler);
    }

    function invariantRegistryRemainsStateless() public view {
        for (uint256 slot; slot < 64; ++slot) {
            assertEq(uint256(vm.load(address(registry), bytes32(slot))), 0);
        }
    }

    function invariantConstantsCannotChange() public view {
        assertEq(registry.VERSION(), 1);
        assertEq(registry.MAX_TTL(), 90 days);
        assertEq(registry.MAX_RECORD_BYTES(), 4096);
    }
}
