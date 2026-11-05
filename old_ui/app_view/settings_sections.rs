//! Secciones del modal de configuración.

use gpui::{AnyElement, Context, div, prelude::*, px};
use gpuikit::{
    elements::{text_field::text_field, typography::text},
    layout::{h_stack, v_stack},
};

use crate::domain::config::{DensidadApp, TemaApp};

use super::{
    AppView, SettingsSection,
    components::{choice_button, info_row, settings_row, toggle_button, toggle_row},
    util::format_filename_preview,
};

impl AppView {
    pub(super) fn render_section_content(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match section {
            SettingsSection::General => self.render_general_section(cx),
            SettingsSection::Extraccion => self.render_extraccion_section(cx),
            SettingsSection::Clasificacion => self.render_clasificacion_section(cx),
            SettingsSection::Excel => self.render_excel_section(cx),
            SettingsSection::Tabla => self.render_tabla_section(cx),
            SettingsSection::Apariencia => self.render_apariencia_section(cx),
            SettingsSection::Atajos => self.render_atajos_section(),
            SettingsSection::Avanzado => self.render_avanzado_section(cx),
        }
    }

    fn render_general_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let recordar = self.state.config.general.recordar_rfc;
        v_stack()
            .gap_3()
            .child(settings_row(
                "RFC propio",
                div()
                    .w(px(220.))
                    .child(text_field(&self.rfc_input, cx).placeholder("Ej. ABC010101ABC")),
            ))
            .child(toggle_row(
                "remember-rfc",
                "Recordar RFC",
                recordar,
                "Activado",
                "Desactivado",
                cx.listener(|this, _, _, cx| {
                    this.state.config.general.recordar_rfc =
                        !this.state.config.general.recordar_rfc;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }

    fn render_extraccion_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let repetir = self
            .state
            .config
            .extraccion
            .repetir_impuestos_en_cada_concepto;
        let tipos = self.state.config.extraccion.incluir_tipos.clone();
        let origen = self.state.config.extraccion.columna_origen_emitida_recibida;

        v_stack()
            .gap_3()
            .child(toggle_row(
                "repetir-impuestos",
                "Impuestos por concepto",
                repetir,
                "Repetir en cada concepto",
                "Solo en la primera fila",
                cx.listener(|this, _, _, cx| {
                    this.state
                        .config
                        .extraccion
                        .repetir_impuestos_en_cada_concepto = !this
                        .state
                        .config
                        .extraccion
                        .repetir_impuestos_en_cada_concepto;
                    cx.notify();
                }),
            ))
            .child(settings_row(
                "Tipos incluidos",
                h_stack()
                    .gap_2()
                    .child(toggle_button(
                        "tipo-i",
                        tipos.i,
                        "I: Sí",
                        "I: No",
                        cx.listener(|this, _, _, cx| {
                            this.state.config.extraccion.incluir_tipos.i =
                                !this.state.config.extraccion.incluir_tipos.i;
                            cx.notify();
                        }),
                    ))
                    .child(toggle_button(
                        "tipo-e",
                        tipos.e,
                        "E: Sí",
                        "E: No",
                        cx.listener(|this, _, _, cx| {
                            this.state.config.extraccion.incluir_tipos.e =
                                !this.state.config.extraccion.incluir_tipos.e;
                            cx.notify();
                        }),
                    ))
                    .child(toggle_button(
                        "tipo-p",
                        tipos.p,
                        "P: Sí",
                        "P: No",
                        cx.listener(|this, _, _, cx| {
                            this.state.config.extraccion.incluir_tipos.p =
                                !this.state.config.extraccion.incluir_tipos.p;
                            cx.notify();
                        }),
                    ))
                    .child(toggle_button(
                        "tipo-n",
                        tipos.n,
                        "N: Sí",
                        "N: No",
                        cx.listener(|this, _, _, cx| {
                            this.state.config.extraccion.incluir_tipos.n =
                                !this.state.config.extraccion.incluir_tipos.n;
                            cx.notify();
                        }),
                    ))
                    .child(toggle_button(
                        "tipo-t",
                        tipos.t,
                        "T: Sí",
                        "T: No",
                        cx.listener(|this, _, _, cx| {
                            this.state.config.extraccion.incluir_tipos.t =
                                !this.state.config.extraccion.incluir_tipos.t;
                            cx.notify();
                        }),
                    )),
            ))
            .child(toggle_row(
                "columna-origen",
                "Origen emitida/recibida",
                origen,
                "Activada",
                "Desactivada",
                cx.listener(|this, _, _, cx| {
                    this.state.config.extraccion.columna_origen_emitida_recibida =
                        !this.state.config.extraccion.columna_origen_emitida_recibida;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }

    fn render_clasificacion_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let categoria = self
            .state
            .config
            .clasificacion
            .categoria_por_defecto
            .clone();
        let nomina = self.state.config.clasificacion.regla_nomina_tipo_n;
        let rfc = self.state.config.clasificacion.regla_ingreso_gasto_por_rfc;

        v_stack()
            .gap_3()
            .child(info_row("Regla por defecto", &categoria))
            .child(toggle_row(
                "regla-nomina",
                "Nómina por tipo N",
                nomina,
                "Activada",
                "Desactivada",
                cx.listener(|this, _, _, cx| {
                    this.state.config.clasificacion.regla_nomina_tipo_n =
                        !this.state.config.clasificacion.regla_nomina_tipo_n;
                    cx.notify();
                }),
            ))
            .child(toggle_row(
                "regla-rfc",
                "Ingreso/Gasto por RFC",
                rfc,
                "Activada",
                "Desactivada",
                cx.listener(|this, _, _, cx| {
                    this.state.config.clasificacion.regla_ingreso_gasto_por_rfc =
                        !this.state.config.clasificacion.regla_ingreso_gasto_por_rfc;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }

    fn render_excel_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let preview = format_filename_preview(&self.state.config.excel.formato_nombre_archivo);
        let congelar = self.state.config.excel.congelar_encabezado;
        let autofiltro = self.state.config.excel.autofiltro;
        let ancho = self.state.config.excel.ancho_automatico;
        let abrir = self.state.config.excel.abrir_carpeta_al_exportar;

        v_stack()
            .gap_3()
            .child(settings_row(
                "Formato Excel",
                div().w(px(260.)).child(
                    text_field(&self.excel_format_input, cx)
                        .placeholder("cfdi_{YYYYMMDD}_{HHmm}.xlsx"),
                ),
            ))
            .child(div().child(text(format!("Vista previa: {preview}")).small().muted()))
            .child(toggle_row(
                "excel-freeze-header",
                "Cabeceras congeladas",
                congelar,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.excel.congelar_encabezado =
                        !this.state.config.excel.congelar_encabezado;
                    cx.notify();
                }),
            ))
            .child(toggle_row(
                "excel-autofilter",
                "Autofiltro",
                autofiltro,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.excel.autofiltro = !this.state.config.excel.autofiltro;
                    cx.notify();
                }),
            ))
            .child(toggle_row(
                "excel-auto-width",
                "Ancho automático",
                ancho,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.excel.ancho_automatico =
                        !this.state.config.excel.ancho_automatico;
                    cx.notify();
                }),
            ))
            .child(toggle_row(
                "open-folder-export",
                "Abrir carpeta al exportar",
                abrir,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.excel.abrir_carpeta_al_exportar =
                        !this.state.config.excel.abrir_carpeta_al_exportar;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }

    fn render_tabla_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let altura = self.state.config.tabla.altura_fila;
        let hover = self.state.config.tabla.mostrar_hover;

        v_stack()
            .gap_3()
            .child(settings_row(
                "Altura de fila",
                h_stack().gap_2().children(
                    [
                        ("row-height-28", "28", 28),
                        ("row-height-32", "32", 32),
                        ("row-height-36", "36", 36),
                        ("row-height-40", "40", 40),
                        ("row-height-44", "44", 44),
                    ]
                    .into_iter()
                    .map(|(id, label, value)| {
                        choice_button(
                            id,
                            label,
                            altura == value,
                            cx.listener(move |this, _, _, cx| {
                                this.state.config.tabla.altura_fila = value;
                                cx.notify();
                            }),
                        )
                    }),
                ),
            ))
            .child(toggle_row(
                "mostrar-hover",
                "Hover",
                hover,
                "Activado",
                "Desactivado",
                cx.listener(|this, _, _, cx| {
                    this.state.config.tabla.mostrar_hover = !this.state.config.tabla.mostrar_hover;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }

    fn render_apariencia_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema_actual = self.state.config.apariencia.tema.clone();
        let densidad_actual = self.state.config.apariencia.densidad.clone();
        let acento = self.state.config.apariencia.color_acento.clone();

        v_stack()
            .gap_3()
            .child(settings_row(
                "Tema",
                h_stack().gap_2().children(
                    [
                        (
                            "theme-system",
                            "Sistema",
                            TemaApp::Sistema,
                            tema_actual == TemaApp::Sistema,
                        ),
                        (
                            "theme-light",
                            "Claro",
                            TemaApp::Claro,
                            tema_actual == TemaApp::Claro,
                        ),
                        (
                            "theme-dark",
                            "Oscuro",
                            TemaApp::Oscuro,
                            tema_actual == TemaApp::Oscuro,
                        ),
                    ]
                    .into_iter()
                    .map(|(id, label, value, active)| {
                        choice_button(
                            id,
                            label,
                            active,
                            cx.listener(move |this, _, _, cx| {
                                this.state.config.apariencia.tema = value.clone();
                                cx.notify();
                            }),
                        )
                    }),
                ),
            ))
            .child(settings_row(
                "Densidad",
                h_stack().gap_2().children(
                    [
                        (
                            "density-compact",
                            "Compacta",
                            DensidadApp::Compacta,
                            densidad_actual == DensidadApp::Compacta,
                        ),
                        (
                            "density-normal",
                            "Normal",
                            DensidadApp::Normal,
                            densidad_actual == DensidadApp::Normal,
                        ),
                    ]
                    .into_iter()
                    .map(|(id, label, value, active)| {
                        choice_button(
                            id,
                            label,
                            active,
                            cx.listener(move |this, _, _, cx| {
                                this.state.config.apariencia.densidad = value.clone();
                                cx.notify();
                            }),
                        )
                    }),
                ),
            ))
            .child(info_row("Color de acento", &acento))
            .into_any_element()
    }

    fn render_atajos_section(&mut self) -> AnyElement {
        v_stack()
            .gap_3()
            .child(info_row(
                "Atajo abrir ZIP",
                &self.state.config.atajos.abrir_zip,
            ))
            .child(info_row(
                "Atajo exportar",
                &self.state.config.atajos.exportar,
            ))
            .child(info_row(
                "Abrir configuración",
                &self.state.config.atajos.abrir_configuracion,
            ))
            .into_any_element()
    }

    fn render_avanzado_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let hilos = self.state.config.avanzado.hilos_de_procesamiento.clone();
        let errores = self.state.config.avanzado.mostrar_errores_detallados;
        let registro = self.state.config.avanzado.registro_de_depuracion;

        v_stack()
            .gap_3()
            .child(settings_row(
                "Hilos de procesamiento",
                h_stack()
                    .gap_2()
                    .children(["Auto", "1", "2", "4"].into_iter().map(|value| {
                        choice_button(
                            match value {
                                "Auto" => "threads-auto",
                                "1" => "threads-1",
                                "2" => "threads-2",
                                _ => "threads-4",
                            },
                            value,
                            hilos == value,
                            cx.listener(move |this, _, _, cx| {
                                this.state.config.avanzado.hilos_de_procesamiento =
                                    value.to_owned();
                                cx.notify();
                            }),
                        )
                    })),
            ))
            .child(toggle_row(
                "mostrar-errores-detallados",
                "Errores detallados",
                errores,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.avanzado.mostrar_errores_detallados =
                        !this.state.config.avanzado.mostrar_errores_detallados;
                    cx.notify();
                }),
            ))
            .child(toggle_row(
                "registro-depuracion",
                "Registro de depuración",
                registro,
                "Sí",
                "No",
                cx.listener(|this, _, _, cx| {
                    this.state.config.avanzado.registro_de_depuracion =
                        !this.state.config.avanzado.registro_de_depuracion;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }
}
