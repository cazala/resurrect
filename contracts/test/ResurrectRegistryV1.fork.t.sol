// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { ResurrectRegistryV1 } from "../src/ResurrectRegistryV1.sol";
import { TestBase } from "./TestBase.sol";

/// @dev Set MAINNET_RPC_URL to exercise deployment/calls against a real-state fork.
contract ResurrectRegistryV1ForkTest is TestBase {
    function testForkDeploymentRemainsPermissionlessAndStateless() public {
        string memory rpcUrl = vm.envOr("MAINNET_RPC_URL", string(""));
        if (bytes(rpcUrl).length == 0) return;

        vm.createSelectFork(rpcUrl);
        ResurrectRegistryV1 registry = new ResurrectRegistryV1();
        address unrelatedPublisher = address(0xA11CE);
        vm.prank(unrelatedPublisher);
        registry.announce(keccak256("resurrect:fork-test:1"), 2, 30 days, hex"010203");

        assertEq(registry.VERSION(), 1);
        assertEq(registry.MAX_TTL(), 90 days);
        for (uint256 slot; slot < 16; ++slot) {
            assertEq(uint256(vm.load(address(registry), bytes32(slot))), 0);
        }
    }
}
