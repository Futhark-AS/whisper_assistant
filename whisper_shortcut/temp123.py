
import tkinter as tk
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg

def plot_function():
    # get function from input field
    function = function_entry.get()
    # create x values
    x = np.linspace(-10, 10, 1000)
    # evaluate function for each x value
    y = eval(function)
    # create figure and axis
    fig, ax = plt.subplots()
    # plot function
    ax.plot(x, y)
    # set labels
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_title('Function Plot')
    # create canvas and show plot
    canvas = FigureCanvasTkAgg(fig, master=root)
    canvas.draw()
    canvas.get_tk_widget().pack()

root = tk.Tk()
root.title("Function Plotter")

# create input field for function
function_label = tk.Label(root, text="Enter function:")
function_label.pack()
function_entry = tk.Entry(root)
function_entry.pack()

# create button to plot function
plot_button = tk.Button(root, text="Plot", command=plot_function)
plot_button.pack()

root.mainloop()


import tkinter as tk
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg

def plot_function():
    # get function from input field
    function = function_entry.get()
    # create x values
    x = np.linspace(-10, 10, 1000)
    # evaluate function for each x value
    y = eval(function)
    # create figure and axis
    fig, ax = plt.subplots()
    # plot function
    ax.plot(x, y)
    # set labels
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_title('Function Plot')
    # create canvas and show plot
    canvas = FigureCanvasTkAgg(fig, master=root)
    canvas.draw()
    canvas.get_tk_widget().pack()

root = tk.Tk()
root.title("Function Plotter")

# create input field for function
function_label = tk.Label(root, text="Enter function:")
function_label.pack()
function_entry = tk.Entry(root)
function_entry.pack()

# create button to plot function
plot_button = tk.Button(root, text="Plot", command=plot_function)
plot_button.pack()

root.mainloop()


import tkinter as tk
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg

def plot_function():
    # get function from input field
    function = function_entry.get()
    # create x values
    x = np.linspace(-10, 10, 1000)
    # evaluate function for each x value
    y = eval(function)
    # create figure and axis
    fig, ax = plt.subplots()
    # plot function
    ax.plot(x, y)
    # set labels
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_title('Function Plot')
    # create canvas and show plot
    canvas = FigureCanvasTkAgg(fig, master=root)
    canvas.draw()
    canvas.get_tk_widget().pack()

root = tk.Tk()
root.title("Function Plotter")

# create input field for function
function_label = tk.Label(root, text="Enter function:")
function_label.pack()
function_entry = tk.Entry(root)
function_entry.pack()

# create button to plot function
plot_button = tk.Button(root, text="Plot", command=plot_function)
plot_button.pack()

root.mainloop()


import tkinter as tk
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg

def plot_function():
    # get function from input field
    function = function_entry.get()
    # create x values
    x = np.linspace(-10, 10, 1000)
    # evaluate function for each x value
    y = eval(function)
    # create figure and axis
    fig, ax = plt.subplots()
    # plot function
    ax.plot(x, y)
    # set labels
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_title('Function Plot')
    # create canvas and show plot
    canvas = FigureCanvasTkAgg(fig, master=root)
    canvas.draw()
    canvas.get_tk_widget().pack()

root = tk.Tk()
root.title("Function Plotter")

# create input field for function
function_label = tk.Label(root, text="Enter function:")
function_label.pack()
function_entry = tk.Entry(root)
function_entry.pack()

# create button to plot function
plot_button = tk.Button(root, text="Plot", command=plot_function)
plot_button.pack()

root.mainloop()

