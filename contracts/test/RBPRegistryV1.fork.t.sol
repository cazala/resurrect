// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { RBPRegistryV1 } from "../src/RBPRegistryV1.sol";
import { TestBase } from "./TestBase.sol";

/// @dev Set MAINNET_RPC_URL to exercise deployment/calls against a real-state fork.
contract RBPRegistryV1ForkTest is TestBase {
    function testForkDeploymentRemainsPermissionlessAndStateless() public {
        string memory rpcUrl = vm.envOr("MAINNET_RPC_URL", string(""));
        if (bytes(rpcUrl).length == 0) return;

        vm.createSelectFork(rpcUrl);
        RBPRegistryV1 registry = new RBPRegistryV1();
        address unrelatedPublisher = address(0xA11CE);
        vm.prank(unrelatedPublisher);
        registry.announce(keccak256("rbp:fork-test:1"), 2, 30 days, hex"010203");

        assertEq(registry.VERSION(), 1);
        assertEq(registry.MAX_TTL(), 90 days);
        for (uint256 slot; slot < 16; ++slot) {
            assertEq(uint256(vm.load(address(registry), bytes32(slot))), 0);
        }
    }
}
