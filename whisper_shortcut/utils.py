import tiktoken
import requests

def num_tokens_from_messages(messages, model="gpt-3.5-turbo-0301"):
    """Returns the number of tokens used by a list of messages."""
    try:
        encoding = tiktoken.encoding_for_model(model)
    except KeyError:
        print("Warning: model not found. Using cl100k_base encoding.")
        encoding = tiktoken.get_encoding("cl100k_base")
    if model == "gpt-3.5-turbo":
        print("Warning: gpt-3.5-turbo may change over time. Returning num tokens assuming gpt-3.5-turbo-0301.")
        return num_tokens_from_messages(messages, model="gpt-3.5-turbo-0301")
    elif model == "gpt-4":
        print("Warning: gpt-4 may change over time. Returning num tokens assuming gpt-4-0314.")
        return num_tokens_from_messages(messages, model="gpt-4-0314")
    elif model == "gpt-3.5-turbo-0301":
        tokens_per_message = 4  # every message follows <|start|>{role/name}\n{content}<|end|>\n
        tokens_per_name = -1  # if there's a name, the role is omitted
    elif model == "gpt-4-0314":
        tokens_per_message = 3
        tokens_per_name = 1
    else:
        raise NotImplementedError(f"""num_tokens_from_messages() is not implemented for model {model}. See https://github.com/openai/openai-python/blob/main/chatml.md for information on how messages are converted to tokens.""")
    num_tokens = 0
    for message in messages:
        num_tokens += tokens_per_message
        for key, value in message.items():
            num_tokens += len(encoding.encode(value))
            if key == "name":
                num_tokens += tokens_per_name
    num_tokens += 3  # every reply is primed with <|start|>assistant<|message|>
    return num_tokens

# let's verify the function above matches the OpenAI API response

# import openai

# example_messages = [
#     {
#         "role": "system",
#         "content": "You are a helpful, pattern-following assistant that translates corporate jargon into plain English.",
#     },
#     {
#         "role": "system",
#         "name": "example_user",
#         "content": "New synergies will help drive top-line growth.",
#     },
#     {
#         "role": "system",
#         "name": "example_assistant",
#         "content": "Things working well together will increase revenue.",
#     },
#     {
#         "role": "system",
#         "name": "example_user",
#         "content": "Let's circle back when we have more bandwidth to touch base on opportunities for increased leverage.",
#     },
#     {
#         "role": "system",
#         "name": "example_assistant",
#         "content": "Let's talk later when we're less busy about how to do better.",
#     },
#     {
#         "role": "user",
#         "content": "This late pivot means we don't have time to boil the ocean for the client deliverable.",
#     },
# ]

# for model in ["gpt-3.5-turbo-0301", "gpt-4-0314"]:
#     print(model)
#     # example token count from the function defined above
#     print(f"{num_tokens_from_messages(example_messages, model)} prompt tokens counted by num_tokens_from_messages().")
#     # example token count from the OpenAI API
#     response = openai.ChatCompletion.create(
#         model=model,
#         messages=example_messages,
#         temperature=0,
#         max_tokens=1  # we're only counting input tokens here, so let's not waste tokens on the output
#     )
#     print(f'{response["usage"]["prompt_tokens"]} prompt tokens counted by the OpenAI API.')
#     print()

def convert_usd_to_nok(amount_usd):
    API_URL = "https://open.er-api.com/v6/latest/USD"

    try:
        response = requests.get(API_URL)
        response.raise_for_status()
        data = response.json()
        usd_to_nok_rate = data["rates"]["NOK"]
        amount_nok = amount_usd * usd_to_nok_rate
        return amount_nok
    except requests.exceptions.RequestException as e:
        print(f"Error fetching exchange rate: {e}")
        return None
    except Exception as e:
        print(f"Error: {e}")
        return None

def print_price(price):
    GREEN = "\033[32m"
    END_COLOR = "\033[0m"
    for key, value in price.items():
        print(f"{GREEN}{key.capitalize()}: ${value:.4f}{END_COLOR}")

    # Print total price
    total_price = sum(price.values())
    nok = convert_usd_to_nok(total_price)
    if nok is not None:
        print(f"{GREEN}Total: ${total_price:.4f} ({nok:.6f} NOK){END_COLOR}")
    else:
        print(f"{GREEN}Total: ${total_price:.4f}{END_COLOR}")