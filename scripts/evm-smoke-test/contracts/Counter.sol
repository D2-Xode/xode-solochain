// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract Counter {
    uint256 public number;
    event Incremented(address indexed by, uint256 newValue);

    function increment() external {
        number += 1;
        emit Incremented(msg.sender, number);
    }

    function setNumber(uint256 newNumber) external {
        number = newNumber;
    }
}
