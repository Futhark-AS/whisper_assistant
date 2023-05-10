text123 = """
package Presentation;

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
import javafx.scene.Scene;
import javafx.scene.image.Image;
import javafx.scene.image.ImageView;
import javafx.scene.layout.HBox;
import javafx.util.Duration;
import javafx.animation.KeyFrame;
import javafx.animation.KeyValue;
import javafx.animation.PauseTransition;
import javafx.animation.Timeline;
import javafx.application.HostServices;
import javafx.fxml.FXML;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.control.Button;
import javafx.scene.control.ProgressBar;
import javafx.scene.text.Text;

public class PartidaController {
    private Partida partida;
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
    private HBox juegoPane;
    @FXML
    private Button abandonarBtn;
    @FXML
    private Text retoprogresstext;
    @FXML
    private Button menuBtn;
    @FXML
    private Button odsEnlaceBtn;
    private String[] odsImages ={"/img/ods.png", "/img/ods1.png", "/img/ods2.png", "/img/ods3.png", "/img/ods4.png",
            "/img/ods5.png", "/img/ods6.png","/img/ods7.png","/img/ods8.png","/img/ods9.png","/img/ods10.png"
            ,"/img/ods11.png","/img/ods12.png","/img/ods13.png","/img/ods14.png","/img/ods15.png","/img/ods16.png",
            "/img/ods17.png"};
    private Timeline timeline;
    private double time=0.5;
    private Sonido sonido;
    private Timer timer;
    // Function that is ran when the user clicks the next reto button
    private Runnable handleNextReto = () -> {
        timeline.stop();
        Reto reto = partida.nextReto();
        //test
        continuar.visibleProperty().set(false);
        continuarConsolidar.visibleProperty().set(false);
        abandonarBtn.setVisible(true);
        if(reto == null) {
            messageText.setText("No hay más retos");
            showGameOver(true);
            startOverButton.visibleProperty().set(true);
            // Si no hay mas retos las partida se ha acabado y se suman al usuario los puntos de la partida
            Services.aumentarPuntuacionUsuario(partida.getCurrentPoints());
            return;
        }

        loadReto(reto);
        setODSImage();
    };
    @FXML
    private ImageView imagenVIda;
    @FXML
    private ImageView imagenSonido;

    private void setODSImage(){
        String rutaOds = odsImages[partida.getCurrentReto().getODS()];
        //System.out.println("el ods es " + partida.getCurrentReto().getODS() + " y su ruta es " +  rutaOds);
        Image image = new Image(getClass().getResourceAsStream(rutaOds));
        odsImage.setImage(image);
    }

    private void showGameOver(boolean finPartidaSinPerderReto){
        abandonarBtn.setVisible(false);
        menuBtn.setVisible(true);
        MessageController messageController = new MessageController();
        Usuario user = services.getUsuario();
            services.updtUsuario(user);
        if(finPartidaSinPerderReto){
            messageController.setTextoMensaje("FELICIDADES, GANASTE!!");
            messageController.setPuntuacionConseguida(partida.getCurrentPoints());
            sonido.getPlayerBackgrounds().stop();
            sonido.getPlayerBackgrounds().playFinPartidaGanada();
            services.updatePartidasGanadas();
        }else{
            messageController.setTextoMensaje("GAME OVER!!");
            points.setText("0");
            consolidatedPoints.setText("0");
            sonido.getPlayerBackgrounds().stop();
            sonido.getPlayerBackgrounds().playFinPartidaPerdida();
            services.updatePartidasPerdidas();
        }

        loadViewInJuegoPane("messageView.fxml", messageController);
    }

    private void showTryAgain() {
        setVidasViewer();
        if (partida.getCurrentReto().getType().equals(RetoType.MULTIPLE_CHOICE)) {
            var messageController = new MessageControllerTryAgain();

            CuatroRespuestas cuatroRespuestas = (CuatroRespuestas) partida.getCurrentReto();
            var answer = cuatroRespuestas.getOptions().get(cuatroRespuestas.getCorrectOption());

            messageController.setTextoMensaje("VUELVE A INTENTARLO");
            messageController.setCorrectAnswer(answer);
            loadViewInJuegoPane("messageViewTryAgain.fxml", messageController);
        } else {
            var messageController = new MessageController();
            messageController.setTextoMensaje("VUELVE A INTENTARLO");
            loadViewInJuegoPane("messageView.fxml", messageController);
        }
        services.updateRetosFallados();
    }

    private void showConsolidatedAbandonar() {
        MessageController messageController = new MessageController();
        messageController.setTextoMensaje("NO ESTÁ MAL!!");
        messageController.setPuntuacionConsolidada(partida.getConsolidatedPoints());
        loadViewInJuegoPane("messageView.fxml", messageController);
    }
    private void setCountDown(long delay){
        timer = new Timer();
        timer.schedule(new TimerTask() {
            @Override
            public void run() {
                sonido.getPlayerCountdowns().playCountDown();

            }
        }, delay);
    }

    // Function that is ran in every reto when the user makes the final decision for
    // the reto

    private Runnable handleEndReto = () -> {
        odsEnlaceBtn.setVisible(false);
        // Una vez el reto ha terminado paramos la barra del tiempo
        timeline.stop();
        if(sonido.getPlayerCountdowns().isPlaying()){
            sonido.getPlayerCountdowns().stop();
        }
        //sonido.getPlayerCountdowns().playCountDown();
        //sonido.getPlayerCountdowns().getMediaPlayer().stop();
        timer.cancel();
        abandonarBtn.setVisible(false);

        if (partida.isJuegoOver()) {
            // Si pierde se guardan los puntos consolidados
            this.actualizarPuntuacionUsuario();
            startOverButton.setVisible(true);
            messageText.setText("Juego over!");
            showGameOver(false);
            Services.resetPicked();

        } else {
                // Una vez terminado el reto se actualiza la puntuacion
                points.setText(partida.getCurrentPoints()+"");

                if (partida.isCurrentRetoAcertado()) {

                   loadCorrectAnswerFrame();
                   services.updateRetosAcertados();

                // Show the next reto button
                continuar.visibleProperty().set(true);
                if (!partida.isConsolidated())
                    continuarConsolidar.visibleProperty().set(true);
            } else {
                // Show the next reto button
                // Si la partida esta consolidada y se falla se actuliza la puntuacion
                // consolidada
                if (partida.isConsolidated()) {
                    System.out.println(111111);
                    consolidatedPoints.setText(partida.getConsolidatedPoints() + "");
                }
                PauseTransition pause = new PauseTransition(Duration.seconds(2));
                pause.setOnFinished(e -> {
                    continuar.visibleProperty().set(true);
                    showTryAgain();
                });

                pause.play();

                // ampliar si tenemos mas tipos de retos, esto es lo de que si fallas 1 te pone
                // otro de la misma diff
                Services.addRandomReto(partida);
            }
        }

    };


    // Show error message related to DB
    private void showDBError() {
        messageText.setText("Error al conectar con la base de datos");
    }

    private void tiempofuncionaporfavor(){
        retoprogresstext.setText("Reto " + (partida.getCurrentRetoIndex()+1) + "/" + partida.getNumRetosTotal());
        //esto ^  ya se que no va aqui pero me salva tiempo de escribir
        //lógica de barra de tiempo
        timeline = new Timeline(
                new KeyFrame(Duration.ZERO, new KeyValue(barratiempo.progressProperty(), 1)),
                new KeyFrame(Duration.minutes(time), e-> {//por ahora el tiempo son 30 segundos para todos los retos, cuenta regresiva
                    // do anything you need here on completion...

                    // Si no es una llamada a la barra por el tiempo de que tienes para consolidar del fxml de felicitaciones
                    // es porque es el tiempo de un reto y se le da por fallado porque se ha terminado el tiempo

                    partida.triggerRetoCompleted(false);
                    timeline.stop();
                    handleEndReto.run();
                    System.out.println("Time ran out");
                    //
                }, new KeyValue(barratiempo.progressProperty(), 0))
        );
        timeline.setCycleCount(Animation.INDEFINITE);
        timeline.stop();
        //fin lógica de barra de tiempo
        setVidasViewer();
    }
    public void setVidasViewer(){
        if(partida.getFails()==0){
            Image image = new Image (getClass().getResourceAsStream("/img/heart.png"));
            imagenVIda.setImage(image);
        }
        else {
            Image image = new Image (getClass().getResourceAsStream("/img/brokenHeart.png"));
            imagenVIda.setImage(image);
        }
    }

    public void initialize() {
        sonido = partida.getSonido();
        Configuracion configuracion = new Configuracion();
        // Ocultamos los campos de consolidar ya que todavia no se ha consolidado
        consolidatedText.setVisible(false);
        consolidatedPoints.setVisible(false);

        // When clicking next reto, show next reto
        continuar.setOnAction(event -> handleNextReto.run());
        setODSImage();

        continuarConsolidar.setOnAction(event -> changeConsolidate());

        // When clicking start over, reset Partida state and show next reto
        startOverButton.setOnAction(event -> {
            // TODO: We should use builder pattern here!
            try {
                sonido.getPlayerBackgrounds().stop();
                sonido.getPlayerBackgrounds().playPartidaClip();

                Services.resetPicked();
                String selector = configuracion.getSelector();
                SelectorRetosStrategy retoFactoria = new RandomRetosStrategy();//por defecto
                if(selector.compareTo("RANDOM")==0){
                    retoFactoria = new RandomRetosStrategy();}
                if(selector.compareTo("SOPA")==0){
                    retoFactoria = new SopaLetrasStrategy();}
                if(selector.compareTo("TEST")==0){
                    retoFactoria = new CuatroRespuestasStrategy();}
                partida = new Partida(retoFactoria);
                partida.resetCurrentRetoIndex();
                partida.setSonido(sonido);
                // Al iniciar una nueva partida se reestablecen los puntos
                points.setText(partida.getCurrentPoints()+"");
                consolidatedPoints.setText(partida.getConsolidatedPoints()+"");
                messageText.setText("Tiempo Restante");
                //Se ocultan los puntos de consolidar
                showConsalidated(partida.isConsolidated());
                partida.previousReto();

            } catch (ExecutionException | InterruptedException e) {
                showDBError();
                e.printStackTrace();
            }
            handleNextReto.run();
            this.startOverButton.visibleProperty().set(false);
            this.menuBtn.visibleProperty().set(false);
            //retoprogresstext.setText("Reto " + (partida.getCurrentRetoIndex()) + "/" + partida.getNumRetosTotal());
        });

        this.continuar.visibleProperty().set(false);
        this.continuarConsolidar.visibleProperty().set(false);
        this.startOverButton.visibleProperty().set(false);
        this.menuBtn.visibleProperty().set(false);

        loadReto(partida.getCurrentReto());

    }

    // Load a reto
    // Given a reto type, load the corresponding FXML file and instantiate
    // corresponding controller with the data it needs to function
    public void loadReto(Reto reto) {
        // Load the corresponding FXML file and controller for the given reto type
        FXMLLoader loader = new FXMLLoader();
        Reto currentReto = partida.getCurrentReto();

        ODSEnlaceMaker odsEnlaceMaker = new ODSEnlaceMaker();

        // Set the event handler for the button click
        odsEnlaceBtn.setOnAction(event -> {
            String url = odsEnlaceMaker.getODSEnlace(currentReto.getODS());
            HostServices hostServices = HostServicesProvider.getInstance().getHostServices();
            hostServices.showDocument(url);
        });
        odsEnlaceBtn.setVisible(true);

        switch (reto.getType()) {
            case MULTIPLE_CHOICE:
                time=0.5;
                tiempofuncionaporfavor();
                setCountDown(20000);
                loader.setLocation(getClass().getResource("cuatroPreguntasView.fxml"));
                CuatroRespuestasController cuatroRespuestasController = new CuatroRespuestasController();

                // If not instance of CuatroRespuestas, show error
                if (!(currentReto instanceof CuatroRespuestas)) {
                    showDBError();
                    System.out.println("Current reto is not instance of CuatroRespuestas");
                    return;
                }

                CuatroRespuestas cuatroRespuestas = (CuatroRespuestas) currentReto;
                cuatroRespuestas.setPartida(partida);
                cuatroRespuestasController.setCuatroPreguntas(cuatroRespuestas);
                cuatroRespuestasController.setHandleEndReto(handleEndReto);

                // Makes JavaFX instantiate the FXML with the given controller
                loader.setControllerFactory(controllerClass -> cuatroRespuestasController);
                timeline.play(); //barra de tiempo
                break;
            case SOPA_LETRAS:
                time=2;
                tiempofuncionaporfavor();
                setCountDown(110000);
                loader.setLocation(getClass().getResource("sopaView.fxml"));
                SopaController sopaController = new SopaController();
                // If not instance of CuatroRespuestas, show error
                if (!(currentReto instanceof SopaLetras)) {
                    showDBError();
                    System.out.println("Current reto is not instance of SopaLetras");
                    return;
                }

                SopaLetras sopaLetras = (SopaLetras) currentReto;
                sopaLetras.setPuntosYTiempo(10, 20);
                sopaLetras.setFilas(15); // NO TOCAR LAS DIMENSIONES, SI PONES MENOS PETA POR EL TAMAÑO DE LAS PALABRAS
                sopaLetras.setColumnas(15); // EL TAMAÑO DE LAS PALABRAS NO PUEDE SER SUPERIOR A 7 NUNCA
                sopaLetras.setSopaMatrix();
                sopaLetras.setPartida(partida);

                sopaController.setSopa(sopaLetras);

                sopaController.setHandleEndReto(handleEndReto);
                loader.setControllerFactory(controllerClass -> sopaController);
                timeline.play(); //barra de tiempo
                break;

            // Add more cases for each type of reto
        }

        loadViewInJuegoPane(loader);
    }

    private void createODSImages(){
        odsImages = new String[18];


    }
    public void setPartida(Partida partida) {
        this.partida = partida;
        partida.resetCurrentRetoIndex();
    }

    public void changeConsolidate(){

        this.partida.consolidateCurrentReto();

        this.showConsalidated(true);

        consolidatedPoints.setText(this.partida.getConsolidatedPoints()+"");

        partida.setConsolidated(true);

        handleNextReto.run();

    }

    private void showConsalidated(boolean show){
        // Mostramos (o no) la puntuacion de consolidar
        consolidatedText.setVisible(show);
        consolidatedText.setManaged(show);
        consolidatedPoints.setManaged(show);
        consolidatedPoints.setVisible(show);
    }

    private void loadCorrectAnswerFrame (){
        FXMLLoader loader = new FXMLLoader();
        // Cuando se acierta la respuesta se carga el pop up de felicitaciones
        loader.setLocation(getClass().getResource("finReto.fxml"));
        PopupController popupController = new PopupController();
        loadViewInJuegoPane("finReto.fxml", popupController);
        startCountDownConsolidar();
        popupController.setConfigurationScreen(partida.getCurrentPoints(),partida.getConsolidatedPoints());
    }

    private void startCountDownConsolidar(){
        time = 0.25;
        timeline = new Timeline(
                new KeyFrame(Duration.ZERO, new KeyValue(barratiempo.progressProperty(), 1)),
                new KeyFrame(Duration.minutes(time), e-> {
                    timeline.stop();
                    handleNextReto.run();

                }, new KeyValue(barratiempo.progressProperty(), 0))
        );
        timeline.setCycleCount(Animation.INDEFINITE);
        timeline.play();
    }

    public void actualizarPuntuacionUsuario(){
        // Logica de aumentar los puntos del usuario con sus puntos consolidados
        if(partida.isConsolidated() && services.getUsuario() != null) {
            Services.aumentarPuntuacionUsuario(partida.getConsolidatedPoints());
        }
    }

    public void setServices(Services servicios){ this.services = servicios;}

    //NO HACE FALTA, LO COGE DE PARTIDA
    public void setSonido(Sonido sonido){
        this.sonido = sonido;
    }

    @FXML
    public void abandonarClicked(Event event) throws IOException {
        // Si abandona se guardan los puntos consolidados
        Usuario user = services.getUsuario();
        services.updtUsuario(user);
        this.actualizarPuntuacionUsuario();
        if(sonido.getPlayerCountdowns().isPlaying()){
            sonido.getPlayerCountdowns().stop();
        }
        timer.cancel();
        timeline.stop();
        showConsolidatedAbandonar();
        Services.resetPicked();
        abandonarBtn.setVisible(false);
        menuBtn.setVisible(true);
    }
    @FXML
    public void volverAlMenuClicked(ActionEvent actionEvent) throws IOException {
        if(!sonido.isMuted()){
            sonido.getPlayerBackgrounds().stop();
            sonido.getPlayerBackgrounds().playInicioClip();
        }
        Scene scene = abandonarBtn.getScene();
        MenuController menuController  = new MenuController();
        menuController.setSonido(sonido);
        menuController.setServices(services);
        timeline.stop();
        Services.resetPicked();
        FXMLLoader loader = new FXMLLoader(HelloApplication.class.getResource("Menu.fxml"));
        loader.setControllerFactory(controllerClass -> menuController); // Makes JavaFX instantiate
        scene.setRoot(loader.load());
    }

    private void loadViewInJuegoPane(FXMLLoader loader) {
        try {
            Parent root = loader.load();
            juegoPane.getChildren().clear();
            juegoPane.getChildren().add(root);
        } catch (IOException e) {
            e.printStackTrace();
        }
    }

    private void loadViewInJuegoPane(String viewName, Object controller) {
        FXMLLoader loader = new FXMLLoader();
        loader.setLocation(getClass().getResource(viewName));
        loader.setControllerFactory(controllerClass -> controller);
        this.loadViewInJuegoPane(loader);
    }

    @FXML
    public void handleSound(Event event) {
        if(sonido.isMuted()){
            Image imagenSoundOn = new Image (getClass().getResourceAsStream("/img/soundOn.png"));
            imagenSonido.setImage(imagenSoundOn);
            System.out.println("Estoy ya muteado");
            sonido.unmuteAll();

        }else{
            sonido.muteAll();
            Image imagenMute = new Image (getClass().getResourceAsStream("/img/muted.png"));
            imagenSonido.setImage(imagenMute);
        }

    }
}

"""

