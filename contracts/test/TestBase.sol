// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

interface Vm {
    function expectRevert(bytes4 selector) external;
    function expectEmit(bool, bool, bool, bool, address) external;
    function prank(address sender) external;
    function warp(uint256 timestamp) external;
    function load(address target, bytes32 slot) external view returns (bytes32);
}

abstract contract TestBase {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    function assertTrue(bool condition) internal pure {
        require(condition, "assertTrue failed");
    }

    function assertFalse(bool condition) internal pure {
        require(!condition, "assertFalse failed");
    }

    function assertEq(uint256 actual, uint256 expected) internal pure {
        require(actual == expected, "uint values differ");
    }

    function assertEq(bytes memory actual, bytes memory expected) internal pure {
        require(keccak256(actual) == keccak256(expected), "byte values differ");
    }
}

abstract contract InvariantBase is TestBase {
    struct FuzzSelector {
        address addr;
        bytes4[] selectors;
    }

    struct FuzzArtifactSelector {
        string artifact;
        bytes4[] selectors;
    }

    struct FuzzInterface {
        address addr;
        string[] artifacts;
    }

    function targetContracts() public view virtual returns (address[] memory targets);

    function excludeContracts() public pure returns (address[] memory values) {
        return values;
    }

    function targetSenders() public pure returns (address[] memory values) {
        return values;
    }

    function excludeSenders() public pure returns (address[] memory values) {
        return values;
    }

    function targetArtifacts() public pure returns (string[] memory values) {
        return values;
    }

    function excludeArtifacts() public pure returns (string[] memory values) {
        return values;
    }

    function targetArtifactSelectors() public pure returns (FuzzArtifactSelector[] memory values) {
        return values;
    }

    function targetSelectors() public pure returns (FuzzSelector[] memory values) {
        return values;
    }

    function excludeSelectors() public pure returns (FuzzSelector[] memory values) {
        return values;
    }

    function targetInterfaces() public pure returns (FuzzInterface[] memory values) {
        return values;
    }
}
