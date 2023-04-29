import pygame
import sys
import random

pygame.init()

# Screen dimensions
WIDTH = 640
HEIGHT = 480

# Colors
WHITE = (255, 255, 255)
GREEN = (0, 255, 0)
RED = (255, 0, 0)

# Snake settings
snake_size = 20
snake_speed = 20

# Initialize screen
screen = pygame.display.set_mode((WIDTH, HEIGHT))
pygame.display.set_caption("Simple Snake Game")

snake_pos = [[100, 50], [90, 50], [80, 50]]
snake_speed = 20
food_pos = [random.randrange(1, (WIDTH//20)) * 20, random.randrange(1, (HEIGHT//20)) * 20]
food_spawn = True
direction = "RIGHT"

clock = pygame.time.Clock()

def game_over():
    pygame.quit()
    sys.exit()

while True:
    for event in pygame.event.get():
        if event.type == pygame.KEYDOWN:
            if event.key == pygame.K_UP and direction != "DOWN":
                direction = "UP"
            if event.key == pygame.K_DOWN and direction != "UP":
                direction = "DOWN"
            if event.key == pygame.K_LEFT and direction != "RIGHT":
                direction = "LEFT"
            if event.key == pygame.K_RIGHT and direction != "LEFT":
                direction = "RIGHT"

    if direction == "UP":
        snake_pos[0][1] -= snake_speed
    if direction == "DOWN":
        snake_pos[0][1] += snake_speed
    if direction == "LEFT":
        snake_pos[0][0] -= snake_speed
    if direction == "RIGHT":
        snake_pos[0][0] += snake_speed

    snake_pos.insert(0, list(snake_pos[0]))

    if snake_pos[0] == food_pos:
        food_spawn = False
    else:
        snake_pos.pop()

    if not food_spawn:
        food_pos = [random.randrange(1, (WIDTH//20)) * 20, random.randrange(1, (HEIGHT//20)) * 20]
    food_spawn = True

    screen.fill(WHITE)

    for pos in snake_pos:
        pygame.draw.rect(screen, GREEN, pygame.Rect(pos[0], pos[1], snake_size, snake_size))

    pygame.draw.rect(screen, RED, pygame.Rect(food_pos[0], food_pos[1], snake_size, snake_size))

    if snake_pos[0][0] >= WIDTH or snake_pos[0][0] < 0 or snake_pos[0][1] >= HEIGHT or snake_pos[0][1] < 0:
        game_over()

    for block in snake_pos[1:]:
        if snake_pos[0] == block:
            game_over()

    pygame.display.flip()
    clock.tick(10)