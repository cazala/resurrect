// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { ResurrectRegistryV1 } from "../src/ResurrectRegistryV1.sol";
import { TestBase } from "./TestBase.sol";

/// @dev Set MAINNET_RPC_URL to exercise deployment/calls against a real-state fork.
contract ResurrectRegistryV1ForkTest is TestBase {
    address internal constant ETHEREUM_MAINNET_REGISTRY =
        0x6F33c332e8251dcd307D85A27fCcAbd85d578910;
    uint256 internal constant ETHEREUM_MAINNET_DEPLOYMENT_BLOCK = 25_882_327;
    bytes32 internal constant ETHEREUM_MAINNET_RUNTIME_BYTECODE_HASH =
        0x0024244f6ad881009b5726d2c1644a3c2aff178852c4d01b1066cd7d9967c109;

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

    function testPublishedEthereumMainnetDeploymentMatchesPinnedMetadata() public {
        string memory rpcUrl = vm.envOr("MAINNET_RPC_URL", string(""));
        if (bytes(rpcUrl).length == 0) return;

        vm.createSelectFork(rpcUrl);
        assertTrue(block.number >= ETHEREUM_MAINNET_DEPLOYMENT_BLOCK);
        assertEq(
            uint256(ETHEREUM_MAINNET_REGISTRY.codehash),
            uint256(ETHEREUM_MAINNET_RUNTIME_BYTECODE_HASH)
        );

        ResurrectRegistryV1 registry = ResurrectRegistryV1(ETHEREUM_MAINNET_REGISTRY);
        assertEq(registry.VERSION(), 1);
        assertEq(registry.MAX_TTL(), 90 days);
        assertEq(registry.MAX_RECORD_BYTES(), 4096);
        for (uint256 slot; slot < 16; ++slot) {
            assertEq(uint256(vm.load(address(registry), bytes32(slot))), 0);
        }
    }
}
