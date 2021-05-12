//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#include <stdio.h>
#include <dlfcn.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include "nvds_msgapi.h"

#define MAX_LEN 256

// Set the number of the events to send during Sync & Asycn send test
const int num_events = 5;

// Set the msg payload for each test sent event
const char SEND_MSG[] = "{ \
   \"messageid\" : \"84a3a0ad-7eb8-49a2-9aa7-104ded6764d0_c788ea9efa50\", \
   \"mdsversion\" : \"1.0\", \
   \"@timestamp\" : \"\", \
   \"place\" : { \
    \"id\" : \"1\", \
    \"name\" : \"HQ\", \
    \"type\" : \"building/garage\", \
    \"location\" : { \
      \"lat\" : 0, \
      \"lon\" : 0, \
      \"alt\" : 0 \
    }, \
    \"aisle\" : { \
      \"id\" : \"C_126_135\", \
      \"name\" : \"Lane 1\", \
      \"level\" : \"P1\", \
      \"coordinate\" : { \
        \"x\" : 1, \
        \"y\" : 2, \
        \"z\" : 3 \
      } \
     }\
    },\
   \"sensor\" : { \
    \"id\" : \"10_110_126_135_A0\", \
    \"type\" : \"Camera\", \
    \"description\" : \"Aisle Camera\", \
    \"location\" : { \
      \"lat\" : 0, \
      \"lon\" : 0, \
      \"alt\" : 0 \
    }, \
    \"coordinate\" : { \
      \"x\" : 0, \
      \"y\" : 0, \
      \"z\" : 0 \
     } \
    } \
   }";

extern "C" NvDsMsgApiHandle nvds_msgapi_connect(char *connection_str, nvds_msgapi_connect_cb_t connect_cb, char *config_path);
extern "C" NvDsMsgApiErrorType nvds_msgapi_send_async(NvDsMsgApiHandle h_ptr, char *topic, const uint8_t *payload,
                                                      size_t nbuf, nvds_msgapi_send_cb_t send_callback, void *user_ptr);
extern "C" NvDsMsgApiErrorType nvds_msgapi_send(NvDsMsgApiHandle conn, char *topic, const uint8_t *payload, size_t nbuf);
extern "C" NvDsMsgApiErrorType nvds_msgapi_subscribe(NvDsMsgApiHandle conn, char **topics, int num_topics, nvds_msgapi_subscribe_request_cb_t cb, void *user_ctx);
extern "C" NvDsMsgApiErrorType nvds_msgapi_disconnect(NvDsMsgApiHandle h_ptr);
extern "C" void nvds_msgapi_do_work(NvDsMsgApiHandle h_ptr);
extern "C" char *nvds_msgapi_getversion(void);
extern "C" char *nvds_msgapi_get_protocol_name(void);
extern "C" NvDsMsgApiErrorType nvds_msgapi_connection_signature(char *connection_str, char *config_path, char *output_str, int max_len);

int send_cb_count = 0;
int consumed_count = 0;

void connect_cb(NvDsMsgApiHandle *h_ptr, NvDsMsgApiEventType ds_evt)
{
}

void send_cb(void *user_ptr, NvDsMsgApiErrorType completion_flag)
{
    if (completion_flag == NVDS_MSGAPI_OK)
        printf("%s successfully \n", (char *)user_ptr);
    else
        printf("%s with failure\n", (char *)user_ptr);
    send_cb_count++;
}

void subscribe_cb(NvDsMsgApiErrorType flag, void *msg, int len, char *topic, void *user_ptr)
{
    int *ptr = (int *)user_ptr;
    if (flag == NVDS_MSGAPI_ERR)
    {
        printf("Error in consuming message[%d] from pravega broker\n", *ptr);
    }
    else
    {
        printf("Consuming message[%d], on topic[%s]. Payload =%.*s\n\n", *ptr, topic, len, (const char *)msg);
    }
    consumed_count++;
}

int main(int argc, char *argv[])
{
    if (argc < 2) 
    {
        printf("Usage: test_pravega_protocol_apapter PRAVEGA_CONTROLLER_URI [PRAVEGA_CFG_FILE].\n");
        return -1;
    }
    char * pravega_controller_uri = argv[1];
    char * pravega_cfg_file = argc > 2 ? argv[2] : NULL;

    printf("Adapter protocol=%s, version=%s\n", nvds_msgapi_get_protocol_name(), nvds_msgapi_getversion());

    char query_conn_signature[MAX_LEN];
    if (nvds_msgapi_connection_signature(pravega_controller_uri, pravega_cfg_file, query_conn_signature, MAX_LEN) != NVDS_MSGAPI_OK)
    {
        printf("Error querying connection signature string. Exiting\n");
        exit(-1);
    }
    printf("Connection signature queried=%s\n", query_conn_signature);

    // set pravega broker appropriately
    NvDsMsgApiHandle conn_handle;
    conn_handle = nvds_msgapi_connect(pravega_controller_uri, (nvds_msgapi_connect_cb_t)connect_cb, pravega_cfg_file);

    if (!conn_handle)
    {
        printf("Connection failed. Exiting\n");
        exit(-1);
    }

    // Subscribe to topics
    const char *topics[] = {"examples/topic1", "examples/topic2"};
    const int num_topics = 2;
    if (nvds_msgapi_subscribe(conn_handle, (char **)topics, num_topics, subscribe_cb, &consumed_count) != NVDS_MSGAPI_OK)
    {
        printf("Pravega subscription to topic[s] failed. Exiting \n");
        exit(-1);
    }

    printf("Proceeding %d synchronized send test...\n", num_events);
    for (int i = 0; i < num_events; i++)
    {
        if (nvds_msgapi_send(conn_handle, (char *)topics[0], (const uint8_t *)SEND_MSG, strlen(SEND_MSG)) != NVDS_MSGAPI_OK)
        {
            printf("Send [%d] failed\n", i);
        }
        else
        {
            printf("Send [%d] completed\n", i);
            sleep(1);
        }
    }

    printf("Proceeding %d asynchronized send test...\n", num_events);
    char ** send_cb_str = new char*[num_events];
    for (int i = 0; i < num_events; i++) 
    {
        send_cb_str[i] = new char[100];
        snprintf(send_cb_str[i], 100, "Async send [%d] complete", i);
    }
    
    for (int i = 0; i < num_events; i++)
    {
        if (nvds_msgapi_send_async(conn_handle, (char *)topics[1], (const uint8_t *)SEND_MSG,
                                   strlen(SEND_MSG), send_cb, send_cb_str[i]) != NVDS_MSGAPI_OK)
            printf("Send [%d] failed\n", i);
        else
            printf("Sending [%d] asynchronously\n", i);
    }

    while (send_cb_count < num_events)
    {
        sleep(1);
        nvds_msgapi_do_work(conn_handle); // need to continuously call do_work to process callbacks
    }
    printf("Disconnecting... in 3 secs\n");
    sleep(3);
    nvds_msgapi_disconnect(conn_handle);
    
    return 0;
}
