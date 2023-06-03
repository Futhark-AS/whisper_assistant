# Tic-Tac-Toe Game

# Define the game board
board = [' ' for x in range(9)]

# Define the players
player1 = 'X'
player2 = 'O'

# Define a function to print the board
def print_board():
    row1 = '|{}|{}|{}|'.format(board[0], board[1], board[2])
    row2 = '|{}|{}|{}|'.format(board[3], board[4], board[5])
    row3 = '|{}|{}|{}|'.format(board[6], board[7], board[8])
    
    print(row1)
    print(row2)
    print(row3)

print_board()
