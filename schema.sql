--
-- PostgreSQL database dump
--

-- Dumped from database version 12.4 (Debian 12.4-3)
-- Dumped by pg_dump version 12.4 (Debian 12.4-3)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: online_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.online_status AS ENUM (
    'dnd',
    'idle',
    'invisible',
    'offline',
    'online'
);


--
-- Name: shipment_carrier; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.shipment_carrier AS ENUM (
    'fedex',
    'ups',
    'usps'
);


--
-- Name: shipment_tracking_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.shipment_tracking_status AS ENUM (
    'unknown',
    'pre_transit',
    'transit',
    'delivered',
    'returned',
    'failure'
);


--
-- Name: row_update_date(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.row_update_date() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
begin
new.update_date = now();
return new;
end;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: playtime_button; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.playtime_button (
    id integer NOT NULL,
    create_date timestamp with time zone DEFAULT now() NOT NULL,
    author_id bigint NOT NULL,
    user_ids bigint[] NOT NULL,
    username character varying(32),
    start_date timestamp with time zone,
    end_date timestamp with time zone,
    start_offset integer NOT NULL
);


--
-- Name: playtime_button_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.playtime_button_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: playtime_button_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.playtime_button_id_seq OWNED BY public.playtime_button.id;


--
-- Name: shipment; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.shipment (
    id integer NOT NULL,
    create_date timestamp with time zone DEFAULT now() NOT NULL,
    update_date timestamp with time zone DEFAULT now() NOT NULL,
    carrier public.shipment_carrier NOT NULL,
    tracking_number character varying(100) NOT NULL,
    author_id bigint NOT NULL,
    channel_id bigint NOT NULL,
    status public.shipment_tracking_status NOT NULL,
    comment character varying(50)
);


--
-- Name: shipment_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.shipment_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: shipment_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.shipment_id_seq OWNED BY public.shipment.id;


--
-- Name: user_presence; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_presence (
    id bigint NOT NULL,
    create_date timestamp with time zone DEFAULT now() NOT NULL,
    user_id bigint NOT NULL,
    status public.online_status NOT NULL,
    game_name character varying(512)
);


--
-- Name: user_presence_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.user_presence_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: user_presence_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.user_presence_id_seq OWNED BY public.user_presence.id;


--
-- Name: playtime_button id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.playtime_button ALTER COLUMN id SET DEFAULT nextval('public.playtime_button_id_seq'::regclass);


--
-- Name: shipment id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipment ALTER COLUMN id SET DEFAULT nextval('public.shipment_id_seq'::regclass);


--
-- Name: user_presence id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_presence ALTER COLUMN id SET DEFAULT nextval('public.user_presence_id_seq'::regclass);


--
-- Name: shipment shipment_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipment
    ADD CONSTRAINT shipment_pkey PRIMARY KEY (id);


--
-- Name: shipment shipment_uk_carrier_number; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.shipment
    ADD CONSTRAINT shipment_uk_carrier_number UNIQUE (carrier, tracking_number);


--
-- Name: user_presence user_presence_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_presence
    ADD CONSTRAINT user_presence_pkey PRIMARY KEY (id);


--
-- Name: shipment shipment_row_update_date; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER shipment_row_update_date BEFORE UPDATE ON public.shipment FOR EACH ROW EXECUTE FUNCTION public.row_update_date();


--
-- Name: TABLE playtime_button; Type: ACL; Schema: public; Owner: -
--

GRANT SELECT,INSERT,UPDATE ON TABLE public.playtime_button TO rustyz;


--
-- Name: SEQUENCE playtime_button_id_seq; Type: ACL; Schema: public; Owner: -
--

GRANT USAGE ON SEQUENCE public.playtime_button_id_seq TO rustyz;


--
-- Name: TABLE shipment; Type: ACL; Schema: public; Owner: -
--

GRANT SELECT,INSERT,UPDATE ON TABLE public.shipment TO rustyz;


--
-- Name: SEQUENCE shipment_id_seq; Type: ACL; Schema: public; Owner: -
--

GRANT USAGE ON SEQUENCE public.shipment_id_seq TO rustyz;


--
-- Name: TABLE user_presence; Type: ACL; Schema: public; Owner: -
--

GRANT SELECT,INSERT ON TABLE public.user_presence TO rustyz;


--
-- Name: SEQUENCE user_presence_id_seq; Type: ACL; Schema: public; Owner: -
--

GRANT USAGE ON SEQUENCE public.user_presence_id_seq TO rustyz;


--
-- PostgreSQL database dump complete
--

