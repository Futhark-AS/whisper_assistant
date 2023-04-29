import pygame
import sys
import random

pygame.init()

# Screen dimensions
WIDTH = 800
HEIGHT = 600

# Colors
WHITE = (255, 255, 255)
GREEN = (0, 255, 0)
RED = (255, 0, 0)

# Snake settings
snake_pos = [[100, 50], [90, 50], [80, 50]]
snake_speed = 10
snake_direction = pygame.K_RIGHT

# Food settings
food_pos = [random.randrange(1, (WIDTH//10)) * 10, random.randrange(1, (HEIGHT//10)) * 10]
food_spawn = True

# Game settings
screen = pygame.display.set_mode((WIDTH, HEIGHT))
pygame.display.set_caption("Snake Game")
clock = pygame.time.Clock()

def game_over():
    pygame.quit()
    sys.exit()

while True:
    for event in pygame.event.get():
        if event.type == pygame.QUIT:
            game_over()
        elif event.type == pygame.KEYDOWN:
            if event.key == pygame.K_UP and snake_direction != pygame.K_DOWN:
                snake_direction = pygame.K_UP
            if event.key == pygame.K_DOWN and snake_direction != pygame.K_UP:
                snake_direction = pygame.K_DOWN
            if event.key == pygame.K_LEFT and snake_direction != pygame.K_RIGHT:
                snake_direction = pygame.K_LEFT
            if event.key == pygame.K_RIGHT and snake_direction != pygame.K_LEFT:
                snake_direction = pygame.K_RIGHT

    if snake_direction == pygame.K_UP:
        snake_pos[0][1] -= 10
    if snake_direction == pygame.K_DOWN:
        snake_pos[0][1] += 10
    if snake_direction == pygame.K_LEFT:
        snake_pos[0][0] -= 10
    if snake_direction == pygame.K_RIGHT:
        snake_pos[0][0] += 10

    snake_pos.insert(0, list(snake_pos[0]))
    if snake_pos[0] == food_pos:
        food_spawn = False
    else:
        snake_pos.pop()

    if not food_spawn:
        food_pos = [random.randrange(1, (WIDTH//10)) * 10, random.randrange(1, (HEIGHT//10)) * 10]
    food_spawn = True

    screen.fill(WHITE)
    for pos in snake_pos:
        pygame.draw.rect(screen, GREEN, pygame.Rect(pos[0], pos[1], 10, 10))

    pygame.draw.rect(screen, RED, pygame.Rect(food_pos[0], food_pos[1], 10, 10))

    if snake_pos[0][0] >= WIDTH or snake_pos[0][0] < 0 or snake_pos[0][1] >= HEIGHT or snake_pos[0][1] < 0:
        game_over()

    for block in snake_pos[1:]:
        if snake_pos[0] == block:
            game_over()

    pygame.display.flip()
    clock.tick(snake_speed)
