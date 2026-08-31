// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import { ResurrectRegistryV1 } from "../src/ResurrectRegistryV1.sol";

interface VmScript {
    function envUint(string calldata name) external view returns (uint256);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

contract DeployResurrectRegistryV1 {
    VmScript internal constant vm =
        VmScript(address(uint160(uint256(keccak256("hevm cheat code")))));

    function run() external returns (ResurrectRegistryV1 registry) {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);
        registry = new ResurrectRegistryV1();
        vm.stopBroadcast();
    }
}

