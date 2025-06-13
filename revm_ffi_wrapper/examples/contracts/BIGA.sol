// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BIGA {
    string public name = "BIGA";
    string public symbol = "BIGA";
    uint8 public decimals = 18;
    uint256 public totalSupply;
    
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    
    // Events
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    
    function mint(address to, uint256 amount) public {
        totalSupply += amount;
        balanceOf[to] += amount;
        emit Transfer(address(0), to, amount);
    }
    
    function transfer(address to, uint256 amount) public returns (bool) {
        require(balanceOf[msg.sender] >= amount, "Insufficient balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        emit Transfer(msg.sender, to, amount);
        return true;
    }
    
    function transferFrom(address from, address to, uint256 amount) public returns (bool) {
        require(balanceOf[from] >= amount, "Insufficient balance");
        require(allowance[from][msg.sender] >= amount, "Insufficient allowance");
        
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        allowance[from][msg.sender] -= amount;
        
        emit Transfer(from, to, amount);
        return true;
    }
    
    function approve(address spender, uint256 amount) public returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }
    
    // Batch transfer function built into the token contract
    function batchTransferSequential(
        address startRecipient,
        uint256 transferCount,
        uint256 amountPerTransfer
    ) public {
        require(balanceOf[msg.sender] >= transferCount * amountPerTransfer, "Insufficient balance");
        
        // Convert startRecipient to uint256 for arithmetic
        uint256 recipientBase = uint256(uint160(startRecipient));
        
        for (uint256 i = 0; i < transferCount; i++) {
            // Generate sequential recipient addresses
            address recipient = address(uint160(recipientBase + i));
            
            // Perform the transfer
            balanceOf[msg.sender] -= amountPerTransfer;
            balanceOf[recipient] += amountPerTransfer;
            
            // Emit transfer event
            emit Transfer(msg.sender, recipient, amountPerTransfer);
        }
    }
} 