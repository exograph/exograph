#!/usr/bin/env python3

# Source of ClaytipDatabaseInitFn, the lambda that is invoked at 
# stack creation time to initialize the database.

import os
import logging
import psycopg2
from psycopg2.extensions import ISOLATION_LEVEL_AUTOCOMMIT
from crhelper import CfnResource

logger = logging.getLogger()

helper = CfnResource(ssl_verify=None)

def handler(event, context):
    logger.info("aws-cf-func invoked")
    helper(event, context)

@helper.create
def create(event, context):
    logger.info("Connecting to db...")

    user=os.getenv("CLAY_DATABASE_USER")
    password=os.getenv("CLAY_DATABASE_PASSWORD")
    host=os.getenv("CLAY_DATABASE_HOST")
    port=os.getenv("CLAY_DATABASE_HOST_PORT")
    dbname=os.getenv("CLAY_DATABASE_NAME")

    logger.info(f"user={user}, host={host}, port={port}, dbname={dbname}")

    conn = psycopg2.connect(
        user=user,
        password=password,
        host=host,
        port=port,
        database="postgres"
    )
    conn.set_isolation_level(ISOLATION_LEVEL_AUTOCOMMIT)

    logger.info("Connected to PostgreSQL instance.")

    logger.info("Reading initialization SQL...")
    sql_file = open("index.sql", "r")
    sql = sql_file.read()
    logger.info("SQL: %s", sql)

    cur = conn.cursor()

    logger.info(f"Creating database {dbname}...")
    cur.execute(f"CREATE DATABASE {dbname};")

    cur.close()
    conn.close()

    logger.info(f"Reconnecting using {dbname}...")
    conn2 = psycopg2.connect(
        user=user,
        password=password,
        host=host,
        port=port,
        database=dbname
    )
    conn2.set_isolation_level(ISOLATION_LEVEL_AUTOCOMMIT)

    logger.info("Executing initialization script...")
    cur2 = conn2.cursor()
    cur2.execute(sql)

    cur2.close()
    conn2.close()

    logger.info("Done.")

@helper.update
@helper.delete
def no_op(_, __):
    pass