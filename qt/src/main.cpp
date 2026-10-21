#include <QApplication>
#include <QStyleFactory>
#include "mainwindow.h"

int main(int argc, char* argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName("PAA Viewer");
    app.setApplicationDisplayName("PAA Viewer & Converter (Qt6)");
    app.setOrganizationName("omp");

    // Apply clean dark theme styling
    QApplication::setStyle(QStyleFactory::create("Fusion"));

    QPalette darkPalette;
    darkPalette.setColor(QPalette::Window, QColor(30, 30, 46));
    darkPalette.setColor(QPalette::WindowText, QColor(205, 214, 244));
    darkPalette.setColor(QPalette::Base, QColor(24, 24, 37));
    darkPalette.setColor(QPalette::AlternateBase, QColor(30, 30, 46));
    darkPalette.setColor(QPalette::ToolTipBase, QColor(205, 214, 244));
    darkPalette.setColor(QPalette::ToolTipText, QColor(17, 17, 27));
    darkPalette.setColor(QPalette::Text, QColor(205, 214, 244));
    darkPalette.setColor(QPalette::Button, QColor(49, 50, 68));
    darkPalette.setColor(QPalette::ButtonText, QColor(205, 214, 244));
    darkPalette.setColor(QPalette::BrightText, Qt::red);
    darkPalette.setColor(QPalette::Link, QColor(137, 180, 250));
    darkPalette.setColor(QPalette::Highlight, QColor(137, 180, 250));
    darkPalette.setColor(QPalette::HighlightedText, QColor(17, 17, 27));
    app.setPalette(darkPalette);

    QString initialFile;
    if (argc > 1) {
        initialFile = QString::fromLocal8Bit(argv[1]);
    }

    MainWindow w(initialFile);
    w.show();

    return app.exec();
}
