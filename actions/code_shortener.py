import sys
import os

project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

import config
import pyperclip

from actions.BaseAction import BaseAction
from pynput import keyboard
from actions.java_code_splitter import JavaCodeTextSplitter
from langchain.chat_models import ChatOpenAI
from langchain.prompts import PromptTemplate
from langchain.chains import LLMChain
from typing import List
from langchain import FewShotPromptTemplate
import threading

def setup_chain() -> LLMChain:
    # create our examples
    examples = [
        {
            "query": """
       import java.io.IOException;
import java.util.Timer;
import java.util.TimerTask;
import java.util.concurrent.ExecutionException;

import BusinessLogic.Entities.*;
import BusinessLogic.Entities.Configuracion;
import BusinessLogic.ODSEnlaceMaker;
import BusinessLogic.Services;
import BusinessLogic.Entities.RandomRetosStrategy;
import BusinessLogic.Entities.SopaLetrasStrategy;
import BusinessLogic.Entities.SelectorRetosStrategy;
import BusinessLogic.Entities.CuatroRespuestasStrategy;
import javafx.animation.Animation;
import javafx.event.ActionEvent;
import javafx.event.Event; 
        """,
            "answer": """Imports packages.""",
        },
        {
            "query": """private Partida partida;
    private Services services;
    @FXML
    private Button continuar;
    @FXML
    private Button continuarConsolidar;
    @FXML
    private Button startOverButton;
    @FXML
    private Text messageText;
    @FXML
    private Text points;
    @FXML
    private Text consolidatedText;
    @FXML
    private Text consolidatedPoints;
    @FXML
    private ProgressBar barratiempo;
    @FXML
    private ImageView odsImage;
    @FXML
    private HBox juegoPane;""",
            "answer": """Declares variables:
Partida partida
Services services
Button continuar
Button continuarConsolidar
Button startOverButton
Text messageText
Text points
Text consolidatedText
Text consolidatedPoints
ProgressBar barratiempo
ImageView odsImage
HBox juegoPane
        """,
        },
        {
            "query": """
                private void showGameOver(boolean finPartidaSinPerderReto){{
        abandonarBtn.setVisible(false);
        menuBtn.setVisible(true);
        MessageController messageController = new MessageController();
        Usuario user = services.getUsuario();
            services.updtUsuario(user);
        if(finPartidaSinPerderReto){{
            ...
        }}else{{
            ...
        }}

        loadViewInJuegoPane("messageView.fxml", messageController);
    }} """,
            "answer": """private void showGameOver(boolean finPartidaSinPerderReto)
Shows the game over screen.""",
        },
    ]

    # create a example template
    example_template = """
    Code: {query}
    Summary: {answer}
    """

    # create a prompt example from above template
    example_prompt = PromptTemplate(
        input_variables=["query", "answer"], template=example_template
    )

    # now break our previous prompt into a prefix and suffix
    # the prefix is our instructions
    prefix = """Extract important information concisely from the Java code segment by describing the code. Retain all names of global variables and methods that could be useful for other functions in the script. The following are good examples:
    """
    # and the suffix our user input and output indicator
    suffix = """
    Code: {query}
    Summary: """

    # now create the few shot prompt template
    few_shot_prompt_template = FewShotPromptTemplate(
        examples=examples,
        example_prompt=example_prompt,
        prefix=prefix,
        suffix=suffix,
        input_variables=["query"],
        example_separator="\n\n",
    )

    llm = ChatOpenAI(temperature=0, model_name="gpt-3.5-turbo", verbose=True)

    return LLMChain(llm=llm, prompt=few_shot_prompt_template)


def code_shortener() -> List[str]:
    text = pyperclip.paste()
    splitter = JavaCodeTextSplitter(max_lines_per_chunk=200, extra_lines_before_split=5)
    split_text = splitter.split_text()

    print("Split text into {} chunks".format(len(split_text)))

    def process_code_segment(chain: LLMChain, code_seg: str, id: int, output: List[str]) -> None:
        resp = chain.run(query=code_seg)
        output.append((resp, id))

    def process_code_segments_concurrently(code_segments: List[str]) -> List[str]:
        chain = setup_chain()
        output = []
        threads = [ ]
        for x in range(len(code_segments)):
            t = threading.Thread(target=process_code_segment, args=(chain, code_segments[x], x, output))
            threads.append(t)

        for thread in threads:
            thread.start()

        for thread in threads:
            thread.join()

        return output

    merged_results = process_code_segments_concurrently(split_text)
    merged_results.sort(key=lambda x: x[1])
    texts = [x[0] for x in merged_results]
    s = "\n\n".join(texts)
    pyperclip.copy(s)
    return merged_results

class CodeShortener(BaseAction):
    def __init__(self, shortcut):
        super().__init__(
            name="code-shortener",
            action=code_shortener,
            description="Whisper Transcribe",
            shortcut=shortcut,
            config={
                "record_input": False,
            },
        )
